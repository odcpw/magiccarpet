using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // DrawTextPauseEndOfLevel_2CE30: owner language, original gameplay font and
    // palette. This presenter never owns or advances a notification timer.
    internal static class SourcePlayerNotificationsPresenter
    {
        internal readonly struct Row
        {
            internal readonly string Text;
            internal readonly int X, Y;
            internal readonly bool PausedLabel;
            internal Row(string text, int x, int y, bool pausedLabel = false)
            { Text = text; X = x; Y = y; PausedLabel = pausedLabel; }
        }

        internal static Row[] Rows(EngineSessionTransport.ParticipantNotification[] notices,
            bool paused, bool mapOrDeathView, bool completed, int lineHeight, Func<int, string> language)
        {
            if (notices == null || notices.Length > 8 || lineHeight <= 0 || language == null)
                throw new InvalidDataException("source participant text binding is incomplete");
            var rows = new List<Row>();
            var startX = mapOrDeathView ? 6 : 132;
            var x = startX;
            var y = mapOrDeathView ? 6 : 50;
            if (paused)
            {
                var label = language(425);
                rows.Add(new Row(label, x, y, true));
                // Source uses a fixed eight-pixel displacement here, not the
                // rendered glyph widths of its proportional gameplay font.
                x += 8 * (label.Length + 2);
            }
            if (completed)
            {
                rows.Add(new Row(language(378) + ". " + language(379) + ".", x, y));
                x = startX;
                y += lineHeight;
            }
            foreach (var notice in notices)
            {
                if (notice == null || string.IsNullOrEmpty(notice.participant) || notice.display_name == null)
                    throw new InvalidDataException("source participant notification is incomplete");
                var selector = notice.kind switch {
                    "participant.died" => 374,
                    "participant.banished" => 283,
                    "participant.icons_map_hidden" => 365,
                    "participant.icons_map_visible" => 366,
                    _ => throw new InvalidDataException("unbound participant notification kind: " + notice.kind)
                };
                // Presence is authoritative. Zero is the final visible frame;
                // filtering it here would shorten every source notice.
                var text = language(selector);
                if (notice.kind is "participant.died" or "participant.banished")
                    text = notice.display_name + " " + text;
                rows.Add(new Row(text, x, y));
                x = startX;
                y += lineHeight;
            }
            return rows.ToArray();
        }

        internal static void Draw(EngineSessionTransport.ParticipantNotification[] notices,
            bool paused, bool mapOrDeathView, bool completed, SourceMenuText text,
            Func<int, string> language, Color noticeColor, Color pausedColor)
        {
            var scale = Mathf.Min(Screen.width / 640f, Screen.height / 480f);
            var view = new Rect((Screen.width - 640 * scale) / 2,
                (Screen.height - 480 * scale) / 2, 640 * scale, 480 * scale);
            foreach (var row in Rows(notices, paused, mapOrDeathView, completed, text.LineAdvance, language))
                text.DrawText(row.Text, row.X, row.Y, view, row.PausedLabel ? pausedColor : noticeColor);
        }
    }
}
