using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Original 640x480 source settings pages. Draft ownership and persistence
    // stay in PlayerPreferencesPresenter; this leaf only draws and edits them.
    internal sealed class SourceSettingsDialog
    {
        internal enum Page { Name, Keys, Joystick, Language }

        internal struct Result
        {
            internal bool Confirm;
            internal bool Cancel;
            internal int SelectedKeySlot;
            internal int BeginKeyCapture;
            internal bool ClearStatus;
            internal string Status;
        }

        private int selectedJoystickRow;
        private bool showDone = true;
        private uint lastDoneBlink;
        private Page scrollPage;
        private int scrollHeight, scrollCloseResult, lastScrollFrame = -1;
        private bool cursorLookupAttempted, customCursorActive, previousCursorVisible, cursorDisabledForOpen;
        private Texture2D sourceCursor;

        internal void Open(Page page)
        {
            Close();
            scrollPage = page;
            scrollHeight = 0;
            scrollCloseResult = 0;
            lastScrollFrame = -1;
            selectedJoystickRow = 0;
            showDone = true;
            lastDoneBlink = 0;
        }

        internal void Close()
        {
            RestoreCursor();
            cursorLookupAttempted = false;
            cursorDisabledForOpen = false;
            sourceCursor = null;
            scrollHeight = 0;
            scrollCloseResult = 0;
            lastScrollFrame = -1;
        }

        internal void RestoreCursor()
        {
            if (customCursorActive) Cursor.visible = previousCursorVisible;
            customCursorActive = false;
            cursorLookupAttempted = true;
            cursorDisabledForOpen = true;
            sourceCursor = null;
        }

        internal Result Draw(OwnerCampaignMenuCache cache, Page page, PlayerPreferences draft,
            int captureKey, int selectedKeySlot, string status, InstalledLocaleSummary[] installedLocales = null)
        {
            var view = SourceView(out var scale);
            var input = Event.current;
            if (page == Page.Language)
                return DrawLanguage(cache, draft, installedLocales, status, view, scale, input);
            if (page == Page.Keys)
                return DrawKeys(cache, draft, captureKey, selectedKeySlot, status, view, scale, input);
            if (cache == null || cache.StartupBackground == null || cache.Startup == null)
                return CancelOnly(input);
            if (page == Page.Name)
                return DrawName(cache, draft, status, view, scale, input);
            return DrawJoystick(cache, draft, status, view, scale, input);
        }

        private Result DrawLanguage(OwnerCampaignMenuCache cache, PlayerPreferences draft,
            InstalledLocaleSummary[] locales, string status, Rect view, float scale, Event input)
        {
            if (cache?.LanguageBackground == null || cache.LanguageCursor == null || locales == null || locales.Length == 0)
                return CancelOnly(input);
            var selected = Array.FindIndex(locales, locale => locale != null &&
                locale.id == (draft.selected_locale ?? cache.DefaultLocaleId));
            if (selected < 0) return CancelOnly(input);
            Texture2D flag;
            try { flag = cache.LanguageFlag(locales[selected].id); }
            catch (InvalidDataException) { return CancelOnly(input); }
            GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), Texture2D.blackTexture,
                ScaleMode.StretchToFill, false);
            GUI.DrawTexture(view, cache.LanguageBackground, ScaleMode.StretchToFill, false);
            DrawBitmap(flag, 263, 134, view, scale);
            if (!string.IsNullOrEmpty(status))
                DrawStatus(DialogText(cache), status, 10, 460, view, Color.white);
            if (input.type == EventType.KeyDown)
            {
                if (input.keyCode == KeyCode.Escape)
                    return Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
                if (input.keyCode is KeyCode.Return or KeyCode.KeypadEnter)
                {
                    draft.selected_locale = locales[selected].id;
                    return Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
                }
                if (input.keyCode is KeyCode.LeftArrow or KeyCode.RightArrow)
                {
                    var delta = input.keyCode == KeyCode.LeftArrow ? -1 : 1;
                    draft.selected_locale = locales[(selected + delta + locales.Length) % locales.Length].id;
                    input.Use();
                }
            }
            else if (input.type == EventType.MouseDown)
            {
                if (input.button == 0)
                {
                    draft.selected_locale = locales[(selected + 1) % locales.Length].id;
                    input.Use();
                }
                else if (input.button == 1)
                {
                    draft.selected_locale = locales[selected].id;
                    return Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
                }
            }
            DrawSourceCursor(cache, view, scale, input.mousePosition, cache.LanguageCursor);
            return EmptyResult();
        }

        private static Result CancelOnly(Event input)
        {
            if (input.type == EventType.KeyDown && input.keyCode == KeyCode.Escape)
            {
                input.Use();
                return new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 };
            }
            return EmptyResult();
        }

        private Result DrawName(OwnerCampaignMenuCache cache, PlayerPreferences draft,
            string status, Rect view, float scale, Event input)
        {
            SourceMenuText text;
            Texture2D scroll;
            try
            {
                text = DialogText(cache);
                scroll = cache.StartupBitmap(72);
            }
            catch (InvalidDataException) { return CancelOnly(input); }
            const int x = 356, y = 112, height = 80;
            AdvanceScroll(Page.Name, height, input);
            var completion = CompleteScrollClose();
            if (completion.Confirm || completion.Cancel) return completion;
            DrawScrollPanel(cache, scroll, x, y, scrollHeight, view, scale);
            if (!ScrollReady(height))
            {
                ConsumeTransitionInput(input);
                return EmptyResult();
            }
            if (!TryLanguage(cache, 420, out var prompt))
                return BeginClosing(DrawCancelOnly(cache, scroll, x, y, height, view, scale, input));
            DrawCentered(cache, text, prompt, x + 10, y + scroll.height + 2, scroll.width - 22, view);
            var nameY = y + height / 2;
            text.DrawText((draft.player_name ?? string.Empty).ToUpperInvariant(), x + 10, nameY, view,
                cache.StartupColor(22, 16, 9));
            if ((Time.frameCount / 6 & 1) == 0)
                text.DrawText("_", x + 10 + AdvanceX(cache, text) * (draft.player_name ?? string.Empty).Length, nameY, view,
                    cache.StartupColor(22, 16, 9));
            DrawStatus(text, status, x + 10, y + height - 29, view, cache.StartupColor(63, 63, 63));

            var buttons = DrawConfirmCancel(cache, scroll, x, y, height, view, scale, input);
            if (buttons.Confirm || buttons.Cancel) return BeginClosing(buttons);
            if (input.type == EventType.KeyDown)
            {
                if (input.keyCode == KeyCode.Escape)
                    return BeginClosing(Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 }));
                if (input.keyCode is KeyCode.Return or KeyCode.KeypadEnter)
                    return BeginClosing(Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 }));
                if (input.keyCode is KeyCode.Backspace or KeyCode.Delete)
                {
                    if (!string.IsNullOrEmpty(draft.player_name))
                        draft.player_name = draft.player_name.Substring(0, draft.player_name.Length - 1);
                    input.Use();
                    return new Result { SelectedKeySlot = -1, BeginKeyCapture = -1, ClearStatus = true };
                }
                var character = input.character;
                if (IsSourceNameCharacter(character) && (draft.player_name?.Length ?? 0) < 12)
                {
                    draft.player_name = (draft.player_name ?? string.Empty) + char.ToUpperInvariant(character);
                    input.Use();
                    return new Result { SelectedKeySlot = -1, BeginKeyCapture = -1, ClearStatus = true };
                }
            }
            return EmptyResult();
        }

        private Result DrawKeys(OwnerCampaignMenuCache cache, PlayerPreferences draft, int captureKey,
            int selectedKeySlot, string status, Rect view, float scale, Event input)
        {
            if (cache?.Settings?.Background == null)
                return CancelOnly(input);

            GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), Texture2D.blackTexture);
            GUI.DrawTexture(view, cache.Settings.Background);
            var text = cache?.Campaign?.DialogText ?? cache?.StartupText;
            if (text == null) return CancelOnly(input);
            var labels = new string[draft.keys.Length];
            for (var slot = 0; slot < labels.Length; slot++)
                if (!TryLanguage(cache, 444 + slot, out labels[slot])) return CancelOnly(input);
            var marker = cache.Settings.Bitmap(107);
            for (var slot = 0; slot < draft.keys.Length; slot++)
            {
                var rowY = 50 + 33 * slot;
                text.DrawText(labels[slot], 212, rowY, view);
                var value = captureKey == slot ? "_" : SourceKeyName(draft.keys[slot]);
                text.DrawText(value, 372, rowY, view);
                if (slot == selectedKeySlot)
                    DrawBitmap(marker, 191, rowY, view, scale);
            }
            if (!string.IsNullOrEmpty(status))
                text.DrawText(status, 10, 460, view, cache.StartupColor(63, 63, 63));
            AdvanceDoneBlink();
            var done = cache.Settings.Bitmap(108);
            var doneRect = SourceRect(view, scale, 283, 381, done.width, done.height);
            if (showDone) DrawBitmap(done, 283, 381, view, scale);

            if (input.type == EventType.KeyDown && captureKey < 0)
            {
                if (input.keyCode == KeyCode.Escape)
                    return Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
                if (input.keyCode is KeyCode.Return or KeyCode.KeypadEnter)
                    return Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
                if (input.keyCode == KeyCode.Space)
                    return Consume(input, new Result { SelectedKeySlot = selectedKeySlot, BeginKeyCapture = selectedKeySlot });
                if (input.keyCode is KeyCode.UpArrow or KeyCode.DownArrow)
                {
                    var delta = input.keyCode == KeyCode.UpArrow ? -1 : 1;
                    var next = (selectedKeySlot + delta + draft.keys.Length) % draft.keys.Length;
                    return Consume(input, new Result { SelectedKeySlot = next, BeginKeyCapture = -1 });
                }
            }
            if (input.type == EventType.MouseDown && input.button == 0)
            {
                for (var slot = 0; slot < draft.keys.Length; slot++)
                {
                    var rowY = 50 + 33 * slot;
                    var hit = SourceRect(view, scale, 191, rowY - 4, 410, marker.height);
                    if (hit.Contains(input.mousePosition))
                        return Consume(input, new Result { SelectedKeySlot = slot, BeginKeyCapture = slot });
                }
                if (doneRect.Contains(input.mousePosition))
                    return Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
            }
            DrawSourceCursor(cache, view, scale, input.mousePosition);
            return EmptyResult();
        }

        private Result DrawJoystick(OwnerCampaignMenuCache cache, PlayerPreferences draft,
            string status, Rect view, float scale, Event input)
        {
            SourceMenuText text;
            Texture2D scroll;
            try
            {
                text = DialogText(cache);
                scroll = cache.StartupBitmap(72);
            }
            catch (InvalidDataException) { return CancelOnly(input); }
            const int x = 74, y = 246, height = 100;
            AdvanceScroll(Page.Joystick, height, input);
            var completion = CompleteScrollClose();
            if (completion.Confirm || completion.Cancel) return completion;
            DrawScrollPanel(cache, scroll, x, y, scrollHeight, view, scale);
            if (!ScrollReady(height))
            {
                ConsumeTransitionInput(input);
                return EmptyResult();
            }
            if (!TryLanguage(cache, 437, out var analogue) || !TryLanguage(cache, 438, out var digital))
                return BeginClosing(DrawCancelOnly(cache, scroll, x, y, height, view, scale, input));
            const int lineHeight = 7;
            var rowY = y + height / 2 - 2 * lineHeight;
            var rowRects = new[] {
                SourceRect(view, scale, x + 10, rowY - 2, scroll.width, lineHeight),
                SourceRect(view, scale, x + 10, rowY + lineHeight - 2, scroll.width, lineHeight)
            };
            var selectedColor = cache.StartupColor(63, 63, 63);
            var idleColor = cache.StartupColor(0, 0, 0);
            text.DrawText(analogue, x + 10, rowY, view, selectedJoystickRow == 0 ? selectedColor : idleColor);
            text.DrawText(digital, x + 10, rowY + lineHeight, view,
                selectedJoystickRow == 1 ? selectedColor : idleColor);
            DrawStatus(text, status, x + 10, y + height - 27, view, cache.StartupColor(63, 63, 63));
            var buttons = DrawConfirmCancel(cache, scroll, x, y, height, view, scale, input);
            if (buttons.Cancel) return BeginClosing(buttons);
            if (buttons.Confirm)
            {
                var result = ConfirmJoystick(draft);
                return result.Confirm ? BeginClosing(result) : result;
            }

            if (input.type == EventType.KeyDown)
            {
                if (input.keyCode == KeyCode.Escape)
                    return BeginClosing(Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 }));
                if (input.keyCode is KeyCode.UpArrow or KeyCode.DownArrow)
                {
                    selectedJoystickRow = 1 - selectedJoystickRow;
                    input.Use();
                    return new Result { SelectedKeySlot = -1, BeginKeyCapture = -1, ClearStatus = true };
                }
                if (input.keyCode is KeyCode.Return or KeyCode.KeypadEnter or KeyCode.Space)
                {
                    var result = Consume(input, ConfirmJoystick(draft));
                    return result.Confirm ? BeginClosing(result) : result;
                }
            }
            if (input.type == EventType.MouseDown && input.button == 0)
            {
                for (var row = 0; row < rowRects.Length; row++)
                {
                    if (!rowRects[row].Contains(input.mousePosition)) continue;
                    selectedJoystickRow = row;
                    input.Use();
                    return new Result {
                        SelectedKeySlot = -1,
                        BeginKeyCapture = -1,
                        ClearStatus = row == 0 && HasJoystick(),
                        Status = row == 1 ? "Digital joystick mode is not supported by this runtime." :
                            !HasJoystick() ? "No joystick detected." : null
                    };
                }
            }
            return EmptyResult();
        }

        private Result ConfirmJoystick(PlayerPreferences draft)
        {
            if (selectedJoystickRow == 1)
                return new Result { SelectedKeySlot = -1, BeginKeyCapture = -1,
                    Status = "Digital joystick mode is not supported by this runtime." };
            if (!HasJoystick())
                return new Result { SelectedKeySlot = -1, BeginKeyCapture = -1,
                    Status = "No joystick detected." };
            draft.input_device = "joystick";
            return new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 };
        }

        private static Result DrawConfirmCancel(OwnerCampaignMenuCache cache, Texture2D scroll,
            int x, int y, int height, Rect view, float scale, Event input)
        {
            var okay = cache.StartupBitmap(70);
            var cancel = cache.StartupBitmap(68);
            var okayX = x + 15;
            var cancelX = x + scroll.width - 12 - cancel.width;
            var buttonY = y + height - okay.height;
            var okayRect = SourceRect(view, scale, okayX, buttonY, okay.width, okay.height);
            var cancelRect = SourceRect(view, scale, cancelX, buttonY, cancel.width, cancel.height);
            DrawBitmap(cache.StartupBitmap(okayRect.Contains(input.mousePosition) ? 71 : 70), okayX, buttonY, view, scale);
            DrawBitmap(cache.StartupBitmap(cancelRect.Contains(input.mousePosition) ? 69 : 68), cancelX, buttonY, view, scale);
            if (input.type != EventType.MouseDown || input.button != 0) return EmptyResult();
            if (okayRect.Contains(input.mousePosition))
                return Consume(input, new Result { Confirm = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
            if (cancelRect.Contains(input.mousePosition))
                return Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
            return EmptyResult();
        }

        private static bool HasJoystick()
        {
            foreach (var name in Input.GetJoystickNames())
                if (!string.IsNullOrWhiteSpace(name)) return true;
            return false;
        }

        private static SourceMenuText DialogText(OwnerCampaignMenuCache cache) =>
            cache?.Campaign?.DialogText ?? cache?.StartupText ??
            throw new InvalidDataException("source settings require an original source font binding");

        private static bool TryLanguage(OwnerCampaignMenuCache cache, int selector, out string value)
        {
            try
            {
                value = cache.LanguageText(selector);
                return !string.IsNullOrEmpty(value);
            }
            catch (InvalidDataException)
            {
                value = null;
                return false;
            }
        }

        private static Result DrawCancelOnly(OwnerCampaignMenuCache cache, Texture2D scroll,
            int x, int y, int height, Rect view, float scale, Event input)
        {
            var cancel = cache.StartupBitmap(68);
            var cancelX = x + scroll.width - 12 - cancel.width;
            var buttonY = y + height - cancel.height;
            var cancelRect = SourceRect(view, scale, cancelX, buttonY, cancel.width, cancel.height);
            DrawBitmap(cache.StartupBitmap(cancelRect.Contains(input.mousePosition) ? 69 : 68),
                cancelX, buttonY, view, scale);
            if (input.type == EventType.KeyDown && input.keyCode == KeyCode.Escape)
                return Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
            if (input.type == EventType.MouseDown && input.button == 0 && cancelRect.Contains(input.mousePosition))
                return Consume(input, new Result { Cancel = true, SelectedKeySlot = -1, BeginKeyCapture = -1 });
            return EmptyResult();
        }

        private void AdvanceScroll(Page page, int maximumHeight, Event input)
        {
            if (scrollPage != page)
            {
                scrollPage = page;
                scrollHeight = 0;
                scrollCloseResult = 0;
                lastScrollFrame = -1;
            }
            if (input.type != EventType.Repaint || lastScrollFrame == Time.frameCount) return;
            lastScrollFrame = Time.frameCount;
            if (scrollCloseResult != 0)
                scrollHeight -= 16;
            else
                scrollHeight = scrollHeight == 0 ? 1 : Math.Min(maximumHeight, scrollHeight + 16);
        }

        private bool ScrollReady(int maximumHeight) => scrollCloseResult == 0 && scrollHeight >= maximumHeight;

        private Result CompleteScrollClose()
        {
            if (scrollCloseResult == 0 || scrollHeight >= 0) return EmptyResult();
            var result = new Result {
                Confirm = scrollCloseResult == 1,
                Cancel = scrollCloseResult == 2,
                SelectedKeySlot = -1,
                BeginKeyCapture = -1
            };
            // A rejected Rust Apply leaves the draft open; resetting to height
            // zero lets the same page animate back in with the user's edits.
            scrollHeight = 0;
            scrollCloseResult = 0;
            lastScrollFrame = Time.frameCount;
            return result;
        }

        private Result BeginClosing(Result result)
        {
            if (result.Confirm) scrollCloseResult = 1;
            else if (result.Cancel) scrollCloseResult = 2;
            return EmptyResult();
        }

        private static void ConsumeTransitionInput(Event input)
        {
            if (input.type is EventType.KeyDown or EventType.KeyUp or EventType.MouseDown or EventType.MouseUp)
                input.Use();
        }

        private static void DrawScrollPanel(OwnerCampaignMenuCache cache, Texture2D scroll,
            int x, int y, int height, Rect view, float scale)
        {
            DrawBitmap(scroll, x, y, view, scale);
            if (height <= 0) return;
            var fill = cache.StartupColor(42, 36, 29);
            var edge = cache.StartupColor(37, 31, 25);
            DrawSolid(view, scale, x + 10, y + scroll.height - 2, scroll.width - 22, height, fill);
            DrawSolid(view, scale, x + 10, y, 1, height + 1, edge);
            DrawSolid(view, scale, x + scroll.width - 12, y, 1, height + 1, edge);
            DrawBitmap(scroll, x, y + height, view, scale);
        }

        private static void DrawCentered(OwnerCampaignMenuCache cache, SourceMenuText text, string value, int x, int y,
            int width, Rect view)
        {
            var advanceX = AdvanceX(cache, text);
            var rows = SourceMenuText.WrapFramedText(value, advanceX, width, false);
            foreach (var row in rows)
            {
                var textX = x + width / 2 - advanceX * row.Length / 2;
                text.DrawText(row, textX, y, view);
                y += 7;
            }
        }

        private static int AdvanceX(OwnerCampaignMenuCache cache, SourceMenuText text) =>
            object.ReferenceEquals(cache?.StartupText, text) ? 7 : 4;

        private static void DrawStatus(SourceMenuText text, string status, int x, int y,
            Rect view, Color color)
        {
            if (!string.IsNullOrEmpty(status)) text.DrawText(status, x, y, view, color);
        }

        private void AdvanceDoneBlink()
        {
            var now = unchecked((uint)(Time.realtimeSinceStartupAsDouble * 100));
            if (unchecked(now - lastDoneBlink) < 50) return;
            lastDoneBlink = now;
            showDone = !showDone;
        }

        private void DrawSourceCursor(OwnerCampaignMenuCache cache, Rect view, float scale, Vector2 mousePosition,
            Texture2D selectedCursor = null)
        {
            if (cursorDisabledForOpen) return;
            if (!cursorLookupAttempted)
            {
                cursorLookupAttempted = true;
                try
                {
                    sourceCursor = selectedCursor ?? cache.Settings.Bitmap(110);
                    previousCursorVisible = Cursor.visible;
                    Cursor.visible = false;
                    customCursorActive = true;
                }
                catch (InvalidDataException) { return; }
            }
            if (sourceCursor == null) return;
            var x = Mathf.Clamp(Mathf.FloorToInt((mousePosition.x - view.x) / scale), 0, 640 - sourceCursor.width);
            var y = Mathf.Clamp(Mathf.FloorToInt((mousePosition.y - view.y) / scale), 0, 480 - sourceCursor.height);
            DrawBitmap(sourceCursor, x, y, view, scale);
        }

        private static bool IsSourceNameCharacter(char value) => value == ' ' ||
            value >= '0' && value <= '9' || value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z';

        internal static string SourceKeyName(string value)
        {
            if (!Enum.TryParse(value, true, out KeyCode key)) return value ?? string.Empty;
            return key switch {
                KeyCode.UpArrow => "UP",
                KeyCode.DownArrow => "DOWN",
                KeyCode.LeftArrow => "LEFT",
                KeyCode.RightArrow => "RIGHT",
                KeyCode.Return => "ENTER",
                KeyCode.LeftControl or KeyCode.RightControl => "CTRL",
                KeyCode.LeftShift => "Left Shift",
                KeyCode.RightShift => "Right Shift",
                KeyCode.LeftAlt or KeyCode.RightAlt => "Alt",
                KeyCode.Q => "Q ", KeyCode.W => "W ", KeyCode.E => "E ",
                KeyCode.T => "T ", KeyCode.Y => "Y ", KeyCode.U => "U ",
                KeyCode.A => "A ", KeyCode.F => "F ", KeyCode.G => "G ",
                KeyCode.H => "H ", KeyCode.J => "J ", KeyCode.K => "K ",
                KeyCode.Z => "Z ", KeyCode.X => "X ", KeyCode.B => "B ",
                KeyCode.N => "N ", KeyCode.Minus or KeyCode.Equals => "- ",
                KeyCode.Semicolon => "; ", KeyCode.Backslash => "# ",
                KeyCode.Comma => ", ", KeyCode.Period => ". ", KeyCode.Slash => "/ ",
            // Remaining schema-supported identifiers have no sub_79E10 glyph;
            // keep their readable enum spelling as a compatibility extension.
                _ => ReadableKeyName(value)
            };
        }

        private static string ReadableKeyName(string value)
        {
            if (value.StartsWith("Alpha", StringComparison.Ordinal) && value.Length == 6)
                return value[5].ToString();
            var result = new System.Text.StringBuilder(value.Length + 4);
            for (var index = 0; index < value.Length; index++)
            {
                var character = value[index];
                if (index > 0 && char.IsUpper(character) && char.IsLower(value[index - 1])) result.Append(' ');
                result.Append(char.ToUpperInvariant(character));
            }
            return result.ToString();
        }

        private static Result Consume(Event input, Result result)
        {
            input.Use();
            return result;
        }

        private static Result EmptyResult() =>
            new() { SelectedKeySlot = -1, BeginKeyCapture = -1 };

        private static Rect SourceView(out float scale)
        {
            scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            return new Rect((Screen.width - 640 * scale) / 2, (Screen.height - 480 * scale) / 2,
                640 * scale, 480 * scale);
        }

        private static Rect SourceRect(Rect view, float scale, int x, int y, int width, int height) =>
            new(view.x + x * scale, view.y + y * scale, width * scale, height * scale);

        private static void DrawBitmap(Texture2D bitmap, int x, int y, Rect view, float scale)
        {
            if (bitmap == null) return;
            GUI.DrawTexture(SourceRect(view, scale, x, y, bitmap.width, bitmap.height), bitmap);
        }

        private static void DrawSolid(Rect view, float scale, int x, int y, int width, int height, Color tint)
        {
            var oldColor = GUI.color;
            GUI.color = tint;
            GUI.DrawTexture(SourceRect(view, scale, x, y, width, height), Texture2D.whiteTexture);
            GUI.color = oldColor;
        }
    }
}
