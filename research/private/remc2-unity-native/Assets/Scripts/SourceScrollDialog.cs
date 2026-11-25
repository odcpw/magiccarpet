using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // MenusAndIntros DrawScrollDialog_7BF20, DrawScrollDialog2_7B660 and
    // the ExitDialog and eight-slot Save/LoadGameDialog consumers. No gameplay is retained
    // here: a closed confirmation returns one typed action to the Rust host.
    internal sealed class SourceScrollDialog
    {
        internal enum Kind { Save, Load, Reset, Exit }
        internal sealed class Completion
        {
            internal readonly Kind Operation;
            internal readonly bool Confirmed;
            internal readonly SourceCampaignSaveSlots.Selection Slot;
            internal Completion(Kind operation, bool confirmed, SourceCampaignSaveSlots.Selection slot)
            { Operation = operation; Confirmed = confirmed; Slot = slot; }
        }
        private readonly Kind kind;
        private readonly bool title;
        private readonly SourceCampaignSaveSlots.Entry[] slots;
        private readonly int x, y, maximumHeight, textSelector;
        private int height, selected, closeResult;
        private string draft = "";
        private bool edited;
        private uint caretTime;

        internal SourceScrollDialog(Kind kind, bool title, string configuredSave)
        {
            this.kind = kind;
            this.title = title;
            if (kind == Kind.Reset)
            { x = 37; y = 348; maximumHeight = 60; textSelector = 467; }
            else if (kind == Kind.Exit)
            { x = 352; y = 26; maximumHeight = 80; textSelector = 407; }
            else
            {
                var load = kind == Kind.Load;
                x = title ? load ? 448 : 78 : load ? 510 : 29;
                y = title ? 160 : 60;
                maximumHeight = 200;
                textSelector = load ? 421 : 422;
                slots = SourceCampaignSaveSlots.Scan(configuredSave);
            }
        }

        internal void ReopenAfterFailure()
        {
            // The source scroll opens over successive presentation passes.
            // Retain the selected slot and edited name while it reopens.
            closeResult = 0;
            height = 0;
        }

        internal Completion Draw(OwnerCampaignMenuCache cache, Rect view, bool advance, uint now, Action click)
        {
            if (advance)
            {
                if (closeResult != 0) height -= 16;
                else height = height == 0 ? 1 : height < maximumHeight ? height + 16 : maximumHeight;
            }
            if (height < 0)
            {
                var confirmed = closeResult == 1 && (kind is Kind.Reset or Kind.Exit || selected != 0);
                return new Completion(kind, confirmed,
                    confirmed && (kind is Kind.Save or Kind.Load) ? new SourceCampaignSaveSlots.Selection(selected, draft) : null);
            }
            if (cache.Campaign?.DialogText == null)
                throw new InvalidDataException("source scroll dialogs require the original FONT1 cache");
            var text = cache.Campaign.DialogText;
            Func<int, Texture2D> bitmap = title ? cache.StartupBitmap : cache.Campaign.Bitmap;
            Func<int, int, int, Color> color = title ? cache.StartupColor : cache.Campaign.NearestColor;
            var scroll = bitmap(title ? 72 : 254);
            var okay = bitmap(title ? 70 : 257);
            var cancel = bitmap(title ? 68 : 255);
            var scale = view.width / 640f;
            Rect At(int px, int py, int width, int h) =>
                new(view.x + px * scale, view.y + py * scale, width * scale, h * scale);
            void DrawBitmap(Texture2D image, int px, int py) =>
                GUI.DrawTexture(At(px, py, image.width, image.height), image);
            void Solid(int px, int py, int width, int h, Color tint)
            {
                var previous = GUI.color;
                GUI.color = tint;
                GUI.DrawTexture(At(px, py, width, h), Texture2D.whiteTexture);
                GUI.color = previous;
            }
            DrawBitmap(scroll, x, y);
            if (height == 0) return null;
            Solid(x + 10, y + scroll.height - 2, scroll.width - 22, height, color(42, 36, 29));
            Solid(x + 10, y, 1, height + 1, color(37, 31, 25));
            Solid(x + scroll.width - 12, y + scroll.height - 2, 1, height + 1, color(37, 31, 25));
            DrawBitmap(scroll, x, y + height);
            var ordinary = color(22, 16, 9);
            var highlight = color(63, 63, 63);
            if (height > 17)
            {
                var rows = SourceMenuText.WrapFramedText(cache.LanguageText(textSelector), 4, scroll.width - 22, false);
                for (var row = 0; row < rows.Length; row++)
                    text.DrawText(rows[row], x + 10 + (scroll.width - 22) / 2 - 4 * rows[row].Length / 2,
                        y + scroll.height + 2 + row * 9, view, ordinary);
            }
            var input = Event.current;
            if (slots != null)
                for (var slot = 1; slot <= SourceCampaignSaveSlots.Count; slot++)
                {
                    if (height <= 16 * slot + 21) continue;
                    var position = At(x + 20, y + 16 + 16 * slot, kind == Kind.Load ? 100 : 90, 16);
                    var usable = kind == Kind.Save || slots[slot - 1].Present;
                    var hovered = height >= maximumHeight && usable && position.Contains(input.mousePosition);
                    var label = slots[slot - 1].Present ? slots[slot - 1].Label : cache.LanguageText(414);
                    var tint = selected == slot || hovered ? highlight : ordinary;
                    if (kind == Kind.Load) text.DrawText(slot + ". " + label, x + 20, y + 16 + 16 * slot, view, tint);
                    else
                    {
                        text.DrawText(slot + ".", x + 20, y + 16 + 16 * slot, view, tint);
                        text.DrawText(selected == slot ? draft : label, x + 32, y + 16 + 16 * slot, view, tint);
                    }
                    if (hovered && closeResult == 0 && input.type == EventType.MouseDown && input.button == 0)
                    {
                        selected = slot;
                        draft = label.ToUpperInvariant();
                        edited = false;
                        input.Use();
                    }
                    if (advance && selected == slot && kind == Kind.Save && unchecked(now - caretTime) >= 12)
                    {
                        text.DrawText("_", x + 32 + 4 * draft.Length, y + 16 + 16 * slot, view, highlight);
                        caretTime = now;
                    }
                }
            if (height >= maximumHeight)
            {
                var okayRect = At(x + 15, y + maximumHeight - okay.height, okay.width, okay.height);
                var cancelRect = At(x + scroll.width - 12 - cancel.width, y + maximumHeight - okay.height,
                    cancel.width, bitmap(title ? 69 : 256).height);
                DrawBitmap(bitmap(title ? okayRect.Contains(input.mousePosition) ? 71 : 70 :
                    okayRect.Contains(input.mousePosition) ? 258 : 257), x + 15, y + maximumHeight - okay.height);
                DrawBitmap(bitmap(title ? cancelRect.Contains(input.mousePosition) ? 69 : 68 :
                    cancelRect.Contains(input.mousePosition) ? 256 : 255),
                    x + scroll.width - 12 - cancel.width, y + maximumHeight - okay.height);
                if (closeResult == 0)
                {
                    if (input.type == EventType.KeyDown && input.keyCode is KeyCode.Return or KeyCode.KeypadEnter)
                        closeResult = 1;
                    else if (input.type == EventType.KeyDown && input.keyCode == KeyCode.Escape)
                        closeResult = 2;
                    else if (input.type == EventType.MouseDown && input.button == 0)
                    {
                        if (okayRect.Contains(input.mousePosition)) closeResult = 1;
                        else if (cancelRect.Contains(input.mousePosition)) closeResult = 2;
                    }
                    else if (kind == Kind.Save && selected != 0 && input.type == EventType.KeyDown)
                        draft = EditName(draft, ref edited, input.keyCode, input.character);
                    if (closeResult != 0) click();
                    if (closeResult != 0 || input.type == EventType.KeyDown) input.Use();
                }
            }
            return null;
        }

        internal static string EditName(string current, ref bool edited, KeyCode key, char value)
        {
            if (key is KeyCode.Backspace or KeyCode.Delete)
            { edited = true; return current.Length == 0 ? current : current.Substring(0, current.Length - 1); }
            if (value != ' ' && (value < '0' || value > '9') && (value < 'A' || value > 'Z') && (value < 'a' || value > 'z'))
                return current;
            if (!edited) { current = ""; edited = true; }
            return current.Length < 15 ? current + char.ToUpperInvariant(value) : current;
        }
    }
}
