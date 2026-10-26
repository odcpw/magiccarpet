using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.Native
{
    // NewGameDraw_7EAE0 / MapMenuPortalsDraw_81760 map help and cursor.
    // This leaf owns only the source map overlay cycle; mission state remains
    // the Rust snapshot and owner mission table.
    internal sealed class SourceCampaignMapHover
    {
        private OwnerCampaignMenuCache.Mission previousMission;
        private OwnerCampaignMenuCache.Mission openMission;
        private uint phaseStarted;
        private bool initialized;
        private bool showingPrevious;

        internal void Reset()
        {
            previousMission = null;
            openMission = null;
            phaseStarted = 0;
            initialized = false;
            showingPrevious = false;
        }

        internal void Draw(GameRuntimeTransport.CampaignMenuSnapshot menu, SourceMapMotion motion,
            Rect view, Vector2 logicalPointer, bool showHelp, uint sourceTick100Hz, bool allowHelp,
            IReadOnlyList<OwnerCampaignMenuCache.Mission> missionOrder, Func<int, string> language,
            Func<int, Texture2D> bitmap, SourceMenuText dialogText)
        {
            if (Event.current.type != EventType.Repaint || view.width <= 0 || view.height <= 0) return;

            GUI.BeginGroup(view);
            try
            {
                var localView = new Rect(0, 0, view.width, view.height);
                if (showHelp && allowHelp && menu != null && menu.phase != "results" &&
                    motion != null && motion.Ready && !motion.Travelling)
                    DrawHelp(menu, motion, localView, sourceTick100Hz, missionOrder, language, dialogText);

                // NewGameDialog_77350 selects HSCREEN bank6 sprite 239 for
                // the campaign map and resets the cursor when that map exits.
                if (logicalPointer.x >= 0 && logicalPointer.x < 640 &&
                    logicalPointer.y >= 0 && logicalPointer.y < 480)
                {
                    var cursor = bitmap(239);
                    var scale = view.width / 640f;
                    GUI.DrawTexture(new Rect(logicalPointer.x * scale, logicalPointer.y * scale,
                        cursor.width * scale, cursor.height * scale), cursor, ScaleMode.StretchToFill, true);
                }
            }
            finally { GUI.EndGroup(); }
        }

        private void DrawHelp(GameRuntimeTransport.CampaignMenuSnapshot menu, SourceMapMotion motion,
            Rect view, uint now, IReadOnlyList<OwnerCampaignMenuCache.Mission> missionOrder,
            Func<int, string> language, SourceMenuText dialogText)
        {
            if (missionOrder == null || language == null || dialogText == null)
                throw new InvalidDataException("source campaign map help requires its ordered mission and FONT1 bindings");

            SelectPortals(menu, missionOrder, out var previous, out var open);
            if (previous == null && open == null)
            {
                Reset();
                return;
            }

            if (!initialized || !SameLevel(previousMission, previous) || !SameLevel(openMission, open))
            {
                previousMission = previous;
                openMission = open;
                phaseStarted = now;
                initialized = true;
                showingPrevious = open == null;
            }

            // The source starts on the open frontier (465), then alternates
            // with the last completed portal (464) after (delta / 100) > 8.
            if (previous != null && open != null && unchecked(now - phaseStarted) / 100u > 8u)
            {
                showingPrevious = !showingPrevious;
                phaseStarted = now;
            }

            var target = showingPrevious ? previous : open;
            if (target == null) target = previous ?? open;
            if (target == null) return;

            var textSelector = ReferenceEquals(target, previous) ? 464 : 465;
            var text = language(textSelector);
            if (string.IsNullOrEmpty(text))
                throw new InvalidDataException("source campaign map help language record is unbound: " + textSelector);

            var binding = new SourceMenuText.HelpBinding
            {
                text_selector = textSelector,
                x = target.portal_x - 80 - motion.X,
                y = target.portal_y - 60 - motion.Y,
                line_x = target.portal_x + 16 - motion.X,
                line_y = target.portal_y - 60 - motion.Y,
                line_end_x = target.portal_x + 16 - motion.X,
                line_end_y = target.portal_y - 4 - motion.Y,
                help_key = 11,
                border_selector = 238,
                line_selector = 264,
            };
            dialogText.DrawHelp(binding, text, view);
        }

        private static void SelectPortals(GameRuntimeTransport.CampaignMenuSnapshot menu,
            IReadOnlyList<OwnerCampaignMenuCache.Mission> missionOrder,
            out OwnerCampaignMenuCache.Mission previous, out OwnerCampaignMenuCache.Mission open)
        {
            var available = new HashSet<string>(menu.available_levels ?? Array.Empty<string>(), StringComparer.Ordinal);
            var completed = new HashSet<string>(menu.completed_levels ?? Array.Empty<string>(), StringComparer.Ordinal);
            previous = null;
            open = null;
            for (var index = 0; index < missionOrder.Count; index++)
            {
                var mission = missionOrder[index];
                if (mission == null)
                    throw new InvalidDataException("source campaign mission order contains an empty row");
                if (string.IsNullOrWhiteSpace(mission.level_id) || !available.Contains(mission.level_id))
                    break;
                if (completed.Contains(mission.level_id))
                {
                    previous = mission;
                    continue;
                }
                open = mission;
                break;
            }
        }

        private static bool SameLevel(OwnerCampaignMenuCache.Mission left, OwnerCampaignMenuCache.Mission right) =>
            string.Equals(left?.level_id, right?.level_id, StringComparison.Ordinal);
    }
}
