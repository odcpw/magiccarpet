using System;
using System.IO;
using System.Text;

namespace MagicCarpet.Native
{
    // DrawCurrentObjectiveTextbox (GameUI.cpp:568-577) selects an owner
    // language record using the authoritative stage carrier, not a semantic
    // task name. The ordinary tables and the secret-level special case come
    // from GameUI.cpp IndexLevelText_DB4EE / LevelEndText_DB507 and
    // DrawCurrentObjectiveTextbox_30630 in the pinned source.
    internal sealed class Mc2ObjectiveText
    {
        private static readonly int[] FirstRecord = {
            0x30, 0x36, 0x3c, 0x42, 0x45, 0x48, 0x4d, 0x4f, 0x56, 0x5c,
            0x61, 0x66, 0x69, 0x6e, 0x73, 0x76, 0x7c, 0x7e, 0x83, 0x85,
            0x88, 0x8c, 0x8f, 0x97, 0x9c
        };
        private static readonly int[] EndRecord = {
            0x35, 0x3b, 0x41, 0x44, 0x47, 0x4c, 0x4e, 0x55, 0x5b, 0x60,
            0x65, 0x68, 0x6d, 0x72, 0x75, 0x7b, 0x7d, 0x82, 0x84, 0x87,
            0x8b, 0x8e, 0x96, 0x9c, 0x9e
        };
        private readonly string[] records;
        private readonly Func<int, string> readRecord;

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
            readRecord = index => index >= 0 && index < records.Length ? records[index] : null;
            ValidateRecords();
        }

        internal Mc2ObjectiveText(OwnerCampaignMenuCache campaignCache)
        {
            if (campaignCache == null) throw new ArgumentNullException(nameof(campaignCache));
            readRecord = campaignCache.LanguageText;
            ValidateRecords();
        }

        internal Mc2ObjectiveText(OwnerCampaignMenuCache.LocaleTextBinding binding)
        {
            if (binding == null) throw new ArgumentNullException(nameof(binding));
            readRecord = binding.Record;
            ValidateRecords();
        }

        private void ValidateRecords()
        {
            for (var level = 0; level < FirstRecord.Length; level++)
                for (var index = FirstRecord[level]; index <= EndRecord[level]; index++)
                    RequireRecord(index);
            RequireRecord(284);
            RequireRecord(285);
        }

        internal static int SourceRecordIndex(ushort internalLevel, byte textSelector, bool isLevelEnd,
            int secretAnnouncementRecord = 0)
        {
            if (internalLevel >= FirstRecord.Length && (internalLevel < 30 || internalLevel > 34))
                throw new ArgumentOutOfRangeException(nameof(internalLevel),
                    "Only ordinary source levels 0 through 24 and secret levels 30 through 34 are bound");
            if (secretAnnouncementRecord != 0)
            {
                if (secretAnnouncementRecord != 284 && secretAnnouncementRecord != 285)
                    throw new ArgumentOutOfRangeException(nameof(secretAnnouncementRecord));
                return secretAnnouncementRecord;
            }
            // Source30..34 use the same objective and end strings regardless
            // of their local objective selector.
            if (internalLevel >= 30 && internalLevel <= 34)
                return isLevelEnd ? EndRecord[10] : FirstRecord[0] + 3;
            return isLevelEnd ? EndRecord[internalLevel] : FirstRecord[internalLevel] + textSelector;
        }

        internal string Resolve(ushort internalLevel, NativeSim.CampaignObjectiveStageSnapshot objective,
            int secretAnnouncementRecord = 0)
        {
            var index = SourceRecordIndex(internalLevel, objective.textSelector, objective.isLevelEnd != 0,
                secretAnnouncementRecord);
            RequireRecord(index);
            return readRecord(index);
        }

        private void RequireRecord(int index)
        {
            if (string.IsNullOrEmpty(readRecord(index)))
                throw new InvalidDataException($"Missing owner objective language record {index}");
        }
    }
}
