using System;
using System.IO;
using System.Text;

namespace MagicCarpet.Native
{
    // DrawCurrentObjectiveTextbox (GameUI.cpp:568-577) selects an owner
    // language record using the authoritative stage carrier, not a semantic
    // task name. These are IndexLevelText_DB4EE / LevelEndText_DB507 rows 0..3.
    internal sealed class Mc2ObjectiveText
    {
        private static readonly int[] FirstRecord = { 0x30, 0x36, 0x3c, 0x42 };
        private static readonly int[] EndRecord = { 0x35, 0x3b, 0x41, 0x44 };
        private readonly string[] records;

        internal Mc2ObjectiveText(OwnerAssetCache commonLanguageCache)
        {
            if (commonLanguageCache == null)
                throw new ArgumentNullException(nameof(commonLanguageCache));
            var bytes = commonLanguageCache.ReadOutput("font/language-l1.records-nul.bin");
            if (bytes.Length == 0 || bytes[bytes.Length - 1] != 0)
                throw new InvalidDataException("Incomplete owner language record table");
            // Preserve each original byte, including punctuation and line
            // breaks, rather than replacing source text with client copy.
            records = Encoding.GetEncoding(28591).GetString(bytes).Split('\0');
            for (var level = 0; level < FirstRecord.Length; level++)
                for (var index = FirstRecord[level]; index <= EndRecord[level]; index++)
                    RequireRecord(index);
        }

        internal static int SourceRecordIndex(ushort internalLevel, byte textSelector, bool isLevelEnd)
        {
            if (internalLevel >= FirstRecord.Length)
                throw new ArgumentOutOfRangeException(nameof(internalLevel),
                    "Only ordinary source levels 0 through 3 are bound");
            return isLevelEnd ? EndRecord[internalLevel] : FirstRecord[internalLevel] + textSelector;
        }

        internal string Resolve(ushort internalLevel, NativeSim.CampaignObjectiveStageSnapshot objective)
        {
            var index = SourceRecordIndex(internalLevel, objective.textSelector, objective.isLevelEnd != 0);
            RequireRecord(index);
            return records[index];
        }

        private void RequireRecord(int index)
        {
            if (index < 0 || index >= records.Length || string.IsNullOrEmpty(records[index]))
                throw new InvalidDataException($"Missing owner objective language record {index}");
        }
    }
}
