using System;

namespace MagicCarpet.Native
{
    // Projects the authoritative campaign result onto the persistent terminal
    // map effect. SourceMapMotion owns the original volcano animation row and
    // indexed art; this leaf decides whether that row is eligible to run.
    internal sealed class SourceCampaignTerminalEffects
    {
        // DrawAndSoundDragonAndFire_81EE0 keys the final volcano row to ordinary
        // portal 24 (zero based), rather than to the total number of completed
        // entries, which can also include secret missions.
        internal const int FinalOrdinaryMissionIndex = 24;

        internal readonly struct FinalMissionBinding
        {
            internal readonly string LevelId;
            internal readonly int OrdinaryMissionIndex;
            internal readonly bool IsSecret;

            internal FinalMissionBinding(string levelId, int ordinaryMissionIndex, bool isSecret)
            {
                LevelId = levelId;
                OrdinaryMissionIndex = ordinaryMissionIndex;
                IsSecret = isSecret;
            }
        }

        private uint lastPresentationPass = uint.MaxValue;
        private FinalMissionBinding lastBinding;
        private bool hasLastBinding;

        internal bool VolcanoEnabled { get; private set; }

        // Call once for each source presentation pass. The completed-level
        // snapshot survives dismissal of the results sheet, so the same source
        // volcano remains enabled when the terminal map is revisited.
        internal bool Step(GameRuntimeTransport.CampaignMenuSnapshot menu,
            FinalMissionBinding finalMission, uint presentationPass)
        {
            if (hasLastBinding && presentationPass == lastPresentationPass &&
                SameBinding(finalMission, lastBinding))
                return VolcanoEnabled;

            lastPresentationPass = presentationPass;
            lastBinding = finalMission;
            hasLastBinding = true;
            VolcanoEnabled = IsEarnedFinalMission(menu, finalMission);
            return VolcanoEnabled;
        }

        internal void Reset()
        {
            lastPresentationPass = uint.MaxValue;
            hasLastBinding = false;
            VolcanoEnabled = false;
        }

        internal static bool IsEarnedFinalMission(
            GameRuntimeTransport.CampaignMenuSnapshot menu, FinalMissionBinding finalMission)
        {
            if (menu == null) return false;
            if ((menu.phase != "results" && menu.phase != "mission_select") ||
                string.IsNullOrWhiteSpace(finalMission.LevelId) ||
                finalMission.OrdinaryMissionIndex != FinalOrdinaryMissionIndex ||
                finalMission.IsSecret)
                return false;

            // Rust's semantic menu snapshot is authoritative. Require the
            // actual final ordinary mission in both the source-available and
            // completed sets; a count, secret result, or presentation request
            // cannot manufacture the ending state.
            return Contains(menu.available_levels, finalMission.LevelId) &&
                Contains(menu.completed_levels, finalMission.LevelId);
        }

        private static bool Contains(string[] levels, string levelId)
        {
            if (levels == null) return false;
            foreach (var level in levels)
                if (string.Equals(level, levelId, StringComparison.Ordinal)) return true;
            return false;
        }

        private static bool SameBinding(FinalMissionBinding left, FinalMissionBinding right) =>
            string.Equals(left.LevelId, right.LevelId, StringComparison.Ordinal) &&
            left.OrdinaryMissionIndex == right.OrdinaryMissionIndex &&
            left.IsSecret == right.IsSecret;
    }
}
