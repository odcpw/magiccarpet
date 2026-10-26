using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Presentation of GameUI DrawPauseMenu_2FD90, DrawVolumeSettings_303D0,
    // DrawInGameOptionsMenu_30050 and DrawOkCancelMenu_30A60. Mutations go
    // through the retained Rust runtime/preferences owners.
    internal sealed class SourcePauseMenu : IDisposable
    {
        internal enum ConfirmationResult { None, Confirm, Cancel }
        [Serializable] internal sealed class ProfileBinding
        {
            public string id, palette_path;
            public int[] colour_indices, building_colours;
            public OwnerCampaignMenuCache.BitmapBinding[] bitmaps;
        }
        [Serializable] internal sealed class Binding
        {
            public SourceMenuText.FontBinding font;
            public ProfileBinding[] profiles;
        }
        private sealed class Profile : IDisposable
        {
            internal readonly Dictionary<int, Texture2D> Bitmaps = new();
            internal readonly Dictionary<int, Texture2D> Disabled = new();
            internal Color[] Palette;
            internal int[] Colours, Building;
            internal SourceMenuText Text;
            public void Dispose()
            {
                Text?.Dispose();
                foreach (var texture in Bitmaps.Values.Concat(Disabled.Values))
                    if (Application.isPlaying) UnityEngine.Object.Destroy(texture);
                    else UnityEngine.Object.DestroyImmediate(texture);
            }
        }
        private readonly Dictionary<string, Profile> profiles = new(StringComparer.Ordinal);
        private readonly Func<int, string> language;
        private CampaignMenuPresenter.Action confirmation;
        private bool options;
        private int volume;
        private static readonly int[] OnText = { 400, 390, 392, 469, 398, 7, 382, 13, 359, 363, 357, 395, 365, 441, 442, 468, 402 };
        private static readonly int[] OffText = { 401, 391, 393, 470, 399, 7, 381, 14, 360, 364, 358, 396, 366, 440, 443, 468, 403 };

        internal SourcePauseMenu(Binding binding, Func<string, int, byte[]> read, Func<int, string> language)
        {
            if (binding?.profiles?.Length != 3 || binding.font == null)
                throw new InvalidDataException("source pause menu binding is incomplete");
            this.language = language;
            try
            {
                foreach (var row in binding.profiles)
                {
                    if (row == null || row.id is not ("night" or "day" or "cave") || row.colour_indices?.Length != 5 ||
                        row.building_colours?.Length != 3 || row.bitmaps?.Length is not (10 or 12) || profiles.ContainsKey(row.id))
                        throw new InvalidDataException("source pause palette/profile binding is invalid");
                    var profile = new Profile { Colours = row.colour_indices, Building = row.building_colours };
                    profiles.Add(row.id, profile);
                    var palette = read(row.palette_path, 768);
                    if (palette.Any(value => value > 63) || row.colour_indices.Concat(row.building_colours).Any(index => index < 0 || index > 255))
                        throw new InvalidDataException("source pause palette is not RGB6");
                    profile.Palette = Enumerable.Range(0, 256).Select(index => (Color)new Color32(
                        (byte)(palette[index * 3] * 4), (byte)(palette[index * 3 + 1] * 4),
                        (byte)(palette[index * 3 + 2] * 4), 255)).ToArray();
                    foreach (var bitmap in row.bitmaps)
                    {
                        if (bitmap == null || bitmap.width <= 0 || bitmap.width > 640 || bitmap.height <= 0 || bitmap.height > 480 ||
                            profile.Bitmaps.ContainsKey(bitmap.selector))
                            throw new InvalidDataException("source pause sprite extent is invalid");
                        profile.Bitmaps.Add(bitmap.selector, OwnerCampaignMenuCache.Texture(
                            read(bitmap.path, checked(bitmap.width * bitmap.height * 4)),
                            bitmap.width, bitmap.height, "MC2 " + row.id + " pause sprite " + bitmap.selector));
                        if (bitmap.disabled_path != null)
                            profile.Disabled.Add(bitmap.selector, OwnerCampaignMenuCache.Texture(
                                read(bitmap.disabled_path, checked(bitmap.width * bitmap.height * 4)),
                                bitmap.width, bitmap.height, "MC2 source twice-shaded pause sprite " + bitmap.selector));
                    }
                    foreach (var selector in new[] { 168, 169, 170, 174, 175, 176, 177, 178, 257, 258 })
                        if (!profile.Bitmaps.ContainsKey(selector)) throw new InvalidDataException("source pause sprite is unbound");
                    if (row.bitmaps.Length == 12 && (!profile.Bitmaps.ContainsKey(85) || !profile.Bitmaps.ContainsKey(86)))
                        throw new InvalidDataException("source sorcerer score sprites are unbound");
                    foreach (var selector in new[] { 174, 175, 176, 177, 178 })
                        if (!profile.Disabled.ContainsKey(selector)) throw new InvalidDataException("source pause shading is unbound");
                    var atlas = OwnerCampaignMenuCache.Texture(read(binding.font.path,
                        checked(binding.font.width * binding.font.height * 4)), binding.font.width,
                        binding.font.height, "MC2 original HFONT3 pause font");
                    try { profile.Text = new SourceMenuText(binding.font, atlas, selector => profile.Bitmaps[selector], Color.black); }
                    catch
                    {
                        if (Application.isPlaying) UnityEngine.Object.Destroy(atlas);
                        else UnityEngine.Object.DestroyImmediate(atlas);
                        throw;
                    }
                }
            }
            catch { Dispose(); throw; }
        }

        internal void Reset() { confirmation = CampaignMenuPresenter.Action.None; options = false; volume = 0; }
        internal SourceMenuText GameplayText(string profile) => profiles.TryGetValue(profile, out var value)
            ? value.Text : throw new InvalidDataException("source gameplay font profile is unbound: " + profile);
        internal Texture2D GameplayBitmap(string profile, int selector) => profiles[profile].Bitmaps.TryGetValue(selector, out var value)
            ? value : throw new InvalidDataException("source gameplay sprite is unbound: " + selector);
        internal Color GameplayColor(string profile, byte index) => profiles[profile].Palette[index];
        internal byte GameplayBlackIndex(string profile) => checked((byte)profiles[profile].Colours[0]);
        internal bool DismissSubmenu()
        {
            if (confirmation == CampaignMenuPresenter.Action.None && !options && volume == 0) return false;
            Reset();
            return true;
        }

        internal static Rect IconRect(int selector, int showWidth = 102, int showHeight = 32)
        {
            // PlayerInput GetPauseMenuCoordinates_2FFE0, default 640 mode.
            var width = showWidth + 2;
            var x = ((640 - width) / 2) & ~1;
            var y = (((400 - 4 * showHeight) / 2) - 60) & ~1;
            return selector switch {
                174 => new Rect(x, y, width, showHeight),
                175 => new Rect(x, y + showHeight, width, showHeight),
                176 => new Rect(x, y + 2 * showHeight, width / 2, showHeight),
                177 => new Rect(x + width / 2, y + 2 * showHeight, width / 2, showHeight),
                178 => new Rect(x, y + 3 * showHeight, width, showHeight),
                _ => throw new ArgumentOutOfRangeException(nameof(selector))
            };
        }

        /// DrawOkCancelMenu_30A60 with SelectedMenuItem=1. Escape requests
        /// this source dialog without setting GAME_PAUSED. The caller owns
        /// its pending action and the runtime to close on confirmation.
        internal ConfirmationResult DrawAbandonConfirmation(string profileId,
            bool mapOpen, bool alive, bool flash, bool acceptInput = true)
        {
            if (!profiles.TryGetValue(profileId, out var profile))
                throw new InvalidDataException("mission has no source confirmation palette: " + profileId);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                640 * scale, 480 * scale);
            return DrawConfirmation(profile, 2, view, mapOpen || !alive ? 6 : 132,
                mapOpen || !alive ? 6 : 50, flash, acceptInput);
        }

        private ConfirmationResult DrawConfirmation(Profile profile, int textSelector,
            Rect view, int textX, int textY, bool flash, bool acceptInput = true)
        {
            var scale = view.width / 640f;
            Rect At(Rect rect) => new(view.x + rect.x * scale, view.y + rect.y * scale,
                rect.width * scale, rect.height * scale);
            profile.Text.DrawText(language(textSelector) + "?", textX, textY, view,
                profile.Palette[profile.Colours[flash ? 2 : 0]]);
            foreach (var (selector, x) in new[] { (257, 270), (258, 320) })
            {
                var texture = profile.Bitmaps[selector];
                GUI.DrawTexture(At(new Rect(x, 104, texture.width, texture.height)), texture);
            }
            if (!acceptInput) return ConfirmationResult.None;
            var result = ConfirmationInput(Event.current, At(new Rect(270, 104, 50, 32)),
                At(new Rect(320, 104, 50, 32)));
            if (result != ConfirmationResult.None) Event.current.Use();
            return result;
        }

        internal static ConfirmationResult ConfirmationInput(Event input, Rect okay, Rect cancel) =>
            ConfirmationInput(input.type, input.keyCode, input.button, input.mousePosition, okay, cancel);

        internal static ConfirmationResult ConfirmationInput(EventType type, KeyCode key,
            int button, Vector2 position, Rect okay, Rect cancel)
        {
            if (type == EventType.KeyDown)
                return key switch {
                    KeyCode.Return or KeyCode.KeypadEnter => ConfirmationResult.Confirm,
                    KeyCode.Escape => ConfirmationResult.Cancel,
                    _ => ConfirmationResult.None
                };
            if (type != EventType.MouseDown || button < 0 || button > 1) return ConfirmationResult.None;
            if (okay.Contains(position)) return ConfirmationResult.Confirm;
            if (cancel.Contains(position)) return ConfirmationResult.Cancel;
            return ConfirmationResult.None;
        }

        internal CampaignMenuPresenter.Action Draw(string profileId, PlayerPreferencesPresenter preferences,
            bool canSave, bool canLoad)
        {
            if (!profiles.TryGetValue(profileId, out var profile))
                throw new InvalidDataException("mission has no source pause palette: " + profileId);
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                640 * scale, 480 * scale);
            Rect At(Rect rect) => new(view.x + rect.x * scale, view.y + rect.y * scale,
                rect.width * scale, rect.height * scale);
            void DrawBitmap(int selector, int x, int y, bool enabled = true)
            {
                var texture = enabled ? profile.Bitmaps[selector] : profile.Disabled[selector];
                GUI.DrawTexture(At(new Rect(x, y, texture.width, texture.height)), texture);
            }
            bool Click(Rect rect) => Event.current.type == EventType.MouseDown &&
                Event.current.button <= 1 && At(rect).Contains(Event.current.mousePosition);
            var prefs = preferences?.Current.preferences;
            if (confirmation != CampaignMenuPresenter.Action.None)
            {
                var selector = confirmation == CampaignMenuPresenter.Action.Save ? 424 : 423;
                var result = DrawConfirmation(profile, selector, view, 132, 50, true);
                if (result == ConfirmationResult.Confirm)
                {
                    var action = confirmation;
                    confirmation = CampaignMenuPresenter.Action.None;
                    return action;
                }
                if (result == ConfirmationResult.Cancel) confirmation = CampaignMenuPresenter.Action.None;
                return CampaignMenuPresenter.Action.None;
            }
            if (options)
            {
                DrawOptions(profile, preferences, view, At, Click, DrawBitmap);
                return CampaignMenuPresenter.Action.None;
            }
            profile.Text.DrawText(language(425), 132, 50, view, profile.Palette[profile.Colours[1]]);
            foreach (var selector in new[] { 174, 175, 176, 177, 178 })
            {
                var enabled = selector switch {
                    174 => canSave, 175 => canLoad, 176 => prefs?.sound_enabled == true,
                    177 => prefs?.music_enabled == true, _ => prefs != null
                };
                var rect = IconRect(selector, profile.Bitmaps[178].width, profile.Bitmaps[178].height);
                // DrawPauseMenu places Music immediately after its 51px art;
                // hit-testing divides the 104px region at52px instead.
                var drawX = selector == 177 ? (int)rect.x - (int)rect.width + profile.Bitmaps[177].width : (int)rect.x;
                DrawBitmap(selector, drawX, (int)rect.y, enabled);
                if (enabled && Click(rect))
                {
                    Event.current.Use();
                    if (selector == 174) confirmation = CampaignMenuPresenter.Action.Save;
                    else if (selector == 175) confirmation = CampaignMenuPresenter.Action.Load;
                    else if (selector == 176 || selector == 177) volume = selector == 176 ? 1 : 2;
                    else options = true;
                }
            }
            if (volume != 0 && prefs != null)
                DrawVolume(profile, preferences, At);
            return CampaignMenuPresenter.Action.None;
        }

        private void DrawVolume(Profile profile, PlayerPreferencesPresenter preferences, Func<Rect, Rect> at)
        {
            var rect = IconRect(178, profile.Bitmaps[178].width, profile.Bitmaps[178].height);
            var x = (int)rect.x;
            var y = (int)(rect.y + rect.height);
            var width = (int)rect.width - 2;
            void Fill(int px, int py, int w, int h, int colour)
            {
                var old = GUI.color;
                GUI.color = profile.Palette[colour];
                GUI.DrawTexture(at(new Rect(px, py, w, h)), Texture2D.whiteTexture);
                GUI.color = old;
            }
            var background = profile.Building[0];
            var light = profile.Building[1];
            var dark = profile.Building[2];
            Fill(x, y, width, 24, background);
            Fill(x, y, width, 2, light); Fill(x, y + 22, width, 2, dark);
            Fill(x, y, 2, 22, light); Fill(x + width - 2, y, 2, 24, dark);
            Fill(x + 4, y + 4, width - 8, 16, background);
            Fill(x + 4, y + 4, width - 8, 2, dark); Fill(x + 4, y + 18, width - 8, 2, light);
            Fill(x + 4, y + 4, 2, 14, dark); Fill(x + width - 6, y + 4, 2, 16, light);
            var value = volume == 1 ? preferences.Current.preferences.sound_volume : preferences.Current.preferences.music_volume;
            Fill(x + 6, y + 6, width - 12, 12, profile.Colours[0]);
            if (value > 0) Fill(x + 6, y + 6, value * (width - 12) / 127, 12, profile.Colours[1]);
            if (Event.current.type is EventType.MouseDown or EventType.MouseDrag &&
                at(new Rect(x, y, width, 24)).Contains(Event.current.mousePosition))
            {
                var sourceX = (Event.current.mousePosition.x - at(new Rect(x + 4, y, 1, 1)).x) / (at(new Rect(0, 0, 640, 1)).width / 640f);
                var setting = Mathf.Clamp((int)sourceX * 127 / ((int)rect.width - 12), 0, 127);
                preferences.ApplySource(draft => { if (volume == 1) draft.sound_volume = setting; else draft.music_volume = setting; });
                Event.current.Use();
            }
            else if (Event.current.type == EventType.MouseDown) { volume = 0; Event.current.Use(); }
        }

        private void DrawOptions(Profile profile, PlayerPreferencesPresenter preferences, Rect view,
            Func<Rect, Rect> at, Func<Rect, bool> click, Action<int, int, int, bool> drawBitmap)
        {
            var prefs = preferences.Current.preferences;
            var longest = OnText.Concat(OffText).Max(index => language(index).Length) * 8;
            var width = 6 * ((longest - 11) / 6 + 1) + 16;
            var x = (640 - width) / 2;
            void Row(int rowX, int rowY, int rowWidth)
            {
                var left = profile.Bitmaps[168]; var middle = profile.Bitmaps[169]; var right = profile.Bitmaps[170];
                drawBitmap(168, rowX, rowY, true);
                drawBitmap(170, rowX + rowWidth - right.width, rowY, true);
                for (var tile = rowX + left.width; tile < rowX + rowWidth - right.width; tile += middle.width)
                    drawBitmap(169, tile, rowY, true);
            }
            for (var index = 0; index < 17; index++)
            {
                var supported = index <= 5 || index is 8 or 9 or 10;
                var enabled = index switch {
                    0 => prefs.show_help, 1 => prefs.sound_enabled, 2 => prefs.music_enabled,
                    3 => prefs.speech_enabled, 4 => prefs.flight_assistance,
                    8 => prefs.reflections, 9 => prefs.sky, 10 => prefs.shadows, _ => false
                };
                var text = language(enabled ? OnText[index] : OffText[index]);
                var y = 67 + index * 18;
                Row(x, y, width);
                var hit = new Rect(x, y, width, 18);
                var hovered = at(hit).Contains(Event.current.mousePosition);
                var colour = profile.Colours[!supported ? 4 : hovered ? 2 : 3];
                profile.Text.DrawText(text, x + (width - 8 * text.Length) / 2, y + 2, view, profile.Palette[colour]);
                if (supported && click(hit))
                {
                    var option = index;
                    var brightnessStep = Event.current.button == 1 ? -1 : 1;
                    preferences.ApplySource(draft => {
                        switch (option) {
                            case 0: draft.show_help = !draft.show_help; break;
                            case 1: draft.sound_enabled = !draft.sound_enabled; break;
                            case 2: draft.music_enabled = !draft.music_enabled; break;
                            case 3: draft.speech_enabled = !draft.speech_enabled; break;
                            case 4: draft.flight_assistance = !draft.flight_assistance; break;
                            case 5:
                                var terrain = profile == profiles["night"] ? 1 : profile == profiles["cave"] ? 2 : 0;
                                // ReadOptionMenuEvents calls ChangeSettings with a3=1, so both ends clamp.
                                draft.brightness[terrain] = Mathf.Clamp(draft.brightness[terrain] + brightnessStep, 0, 4);
                                break;
                            case 8: draft.reflections = !draft.reflections; break;
                            case 9: draft.sky = !draft.sky; break;
                            case 10: draft.shadows = !draft.shadows; break;
                        }
                    });
                    Event.current.Use();
                }
            }
            var okayX = (width - 84) / 2 + (640 - width) / 2;
            Row(okayX, 377, 84);
            profile.Text.DrawText("OK", okayX + 33, 379, view, profile.Palette[profile.Colours[3]]);
            if (click(new Rect(okayX, 377, 84, 18))) { options = false; Event.current.Use(); }
        }

        public void Dispose()
        {
            foreach (var profile in profiles.Values) profile.Dispose();
            profiles.Clear();
        }
    }
}
