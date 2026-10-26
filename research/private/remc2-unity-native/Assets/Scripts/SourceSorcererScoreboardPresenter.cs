using System;
using System.Globalization;
using UnityEngine;

namespace MagicCarpet.Native
{
    // EventsFunctions.cpp::DrawSorcererScores_2D1D0. The root owns the
    // player-information menu gate and supplies its retained gameplay font.
    internal sealed class SourceSorcererScoreboardPresenter
    {
        private const int SourceWidth = 640;
        private const int SourceHeight = 480;
        private const int SorcererTile = 85;
        private const int SorcererTileMini = 86;

        internal bool Draw(
            EngineSessionTransport.SourceScoreboard scoreboard,
            SourceMenuText sourceText,
            Func<int, Texture2D> bitmap,
            Func<int, Vector2Int> size,
            Func<byte, Color> paletteColor,
            byte selectedOutlinePaletteIndex)
        {
            if (!TrySourceView(out var view)) return false;
            if (sourceText == null) return false;
            if (scoreboard == null || scoreboard.players == null ||
                !string.IsNullOrEmpty(scoreboard.projection_error))
                return DrawUnavailable(sourceText, view);
            if (scoreboard.players.Length == 0) return false;
            if (bitmap == null || size == null || paletteColor == null)
                return DrawUnavailable(sourceText, view);

            var previousSlot = -1;
            foreach (var player in scoreboard.players)
            {
                if (player == null || player.source_slot >= 8 || player.source_slot <= previousSlot ||
                    string.IsNullOrEmpty(player.display_name) || !IsOwnerByteText(player.display_name) ||
                    player.kills8 == null || player.kills8.Length != 8)
                    return DrawUnavailable(sourceText, view);
                previousSlot = player.source_slot;
            }

            var largeSize = size(SorcererTile);
            var miniSize = size(SorcererTileMini);
            var largeBitmap = bitmap(SorcererTile);
            var miniBitmap = bitmap(SorcererTileMini);
            if (largeBitmap == null || miniBitmap == null || largeSize.x <= 8 || largeSize.y <= 8 ||
                miniSize.x <= 8 || miniSize.y <= 8)
                return DrawUnavailable(sourceText, view);

            var scale = view.width / SourceWidth;
            var positionX = (SourceWidth -
                (scoreboard.players.Length * miniSize.x + miniSize.x)) / 2f;
            var positionY = (SourceHeight -
                scoreboard.players.Length * largeSize.y) / 2f;
            var selectedOutline = paletteColor(selectedOutlinePaletteIndex);
            var oldGuiColor = GUI.color;
            try
            {
                foreach (var player in scoreboard.players)
                {
                    DrawSprite(largeBitmap, largeSize, positionX, positionY, view, scale);
                    DrawOutline(positionX + 4, positionY + 4, largeSize.x - 8, largeSize.y - 8,
                        paletteColor(player.outline_palette_index), view, scale);
                    sourceText.DrawText(player.display_name, (int)(positionX + 8),
                        (int)(positionY + 6), view, paletteColor(player.text_palette_index));
                    sourceText.DrawText(player.max_mana.ToString(CultureInfo.InvariantCulture),
                        (int)(positionX + 8), (int)(positionY + 20), view,
                        paletteColor(player.text_palette_index));

                    var miniX = positionX + largeSize.x;
                    foreach (var opponent in scoreboard.players)
                    {
                        DrawSprite(miniBitmap, miniSize, miniX, positionY, view, scale);
                        if (opponent.source_slot == player.source_slot)
                        {
                            DrawOutline(miniX + 4, positionY + 4, miniSize.x - 8, miniSize.y - 8,
                                selectedOutline, view, scale);
                        }
                        else
                        {
                            DrawOutline(miniX + 4, positionY + 4, miniSize.x - 8, miniSize.y - 8,
                                paletteColor(opponent.outline_palette_index), view, scale);
                            var kills = player.kills8[opponent.source_slot]
                                .ToString("D3", CultureInfo.InvariantCulture);
                            sourceText.DrawText(kills, (int)(miniX + 8), (int)(positionY + 10), view,
                                paletteColor(opponent.text_palette_index));
                        }
                        miniX += miniSize.x;
                    }

                    positionY += miniSize.y;
                }
                return true;
            }
            finally { GUI.color = oldGuiColor; }
        }

        private static bool TrySourceView(out Rect view)
        {
            var scale = Mathf.Min(Screen.width / (float)SourceWidth,
                Screen.height / (float)SourceHeight);
            if (scale <= 0)
            {
                view = default;
                return false;
            }
            var width = SourceWidth * scale;
            var height = SourceHeight * scale;
            view = new Rect((Screen.width - width) / 2f, (Screen.height - height) / 2f, width, height);
            return true;
        }

        private static bool DrawUnavailable(SourceMenuText sourceText, Rect view)
        {
            sourceText.DrawText("PLAYER INFORMATION UNAVAILABLE", 8, 8, view, Color.white);
            return false;
        }

        private static bool IsOwnerByteText(string value)
        {
            foreach (var character in value)
                if (character == 0 || character > byte.MaxValue)
                    return false;
            return true;
        }

        private static void DrawSprite(Texture2D texture, Vector2Int extent,
            float x, float y, Rect view, float scale)
        {
            GUI.color = Color.white;
            GUI.DrawTexture(new Rect(view.x + x * scale, view.y + y * scale,
                extent.x * scale, extent.y * scale), texture, ScaleMode.StretchToFill, true);
        }

        private static void DrawOutline(float x, float y, int width, int height,
            Color color, Rect view, float scale)
        {
            GUI.color = color;
            var left = view.x + x * scale;
            var top = view.y + y * scale;
            var w = width * scale;
            var h = height * scale;
            GUI.DrawTexture(new Rect(left, top, w, scale), Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(left, top + h - scale, w, scale), Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(left, top + scale, scale, h - 2 * scale), Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(left + w - scale, top + scale, scale, h - 2 * scale), Texture2D.whiteTexture);
        }
    }
}
