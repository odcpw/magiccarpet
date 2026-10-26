using System;
using System.IO;
using System.Text;

namespace MagicCarpet.Native
{
    // Host storage for the source's eight named campaign slots. The distinct
    // Rust-slot header is not a legacy .GAM save: gameplay remains the exact
    // receipt-qualified replay bytes produced and consumed by Rust.
    internal static class SourceCampaignSaveSlots
    {
        internal const int Count = 8;
        private static readonly byte[] Magic = Encoding.ASCII.GetBytes("MC2RSV01");
        private const int HeaderBytes = 28;
        internal sealed class Selection
        {
            internal readonly int Index;
            internal readonly string Label;
            internal Selection(int index, string label) { Index = index; Label = label; }
        }
        internal readonly struct Entry
        {
            internal readonly bool Present;
            internal readonly string Label;
            internal Entry(bool present, string label) { Present = present; Label = label; }
        }

        internal static string PathFor(string configuredSave, int index)
        {
            if (index < 1 || index > Count) throw new ArgumentOutOfRangeException(nameof(index));
            if (string.IsNullOrWhiteSpace(configuredSave)) throw new InvalidDataException("campaign save path is unconfigured");
            return Path.Combine(Path.GetDirectoryName(Path.GetFullPath(configuredSave)), "SAVE", $"SAVE{index}.mcrsave");
        }

        internal static Entry[] Scan(string configuredSave)
        {
            var entries = new Entry[Count];
            for (var index = 1; index <= Count; index++)
            {
                var path = PathFor(configuredSave, index);
                if (!File.Exists(path)) continue;
                try
                {
                    using var stream = File.OpenRead(path);
                    entries[index - 1] = new Entry(true, ReadHeader(stream));
                }
                // CreateOrOpenFile returning null or an invalid source
                // signature leaves the corresponding row empty. Selecting
                // Save/Load still reports its actual host I/O failure.
                catch (IOException) { }
                catch (UnauthorizedAccessException) { }
            }
            return entries;
        }

        private static string ReadHeader(Stream stream)
        {
            if (stream.Length <= HeaderBytes || stream.Length > HeaderBytes + (long)GameRuntimeTransport.MaximumReplaySaveJsonBytes)
                throw new InvalidDataException("campaign slot extent is invalid");
            var header = new byte[HeaderBytes];
            var offset = 0;
            while (offset < header.Length)
            {
                var count = stream.Read(header, offset, header.Length - offset);
                if (count == 0) throw new InvalidDataException("campaign slot header is truncated");
                offset += count;
            }
            for (var index = 0; index < Magic.Length; index++)
                if (header[index] != Magic[index]) throw new InvalidDataException("unsupported campaign slot format");
            var end = Array.IndexOf(header, (byte)0, Magic.Length);
            if (end < Magic.Length || end > Magic.Length + 15)
                throw new InvalidDataException("campaign slot label is invalid");
            var label = Encoding.ASCII.GetString(header, Magic.Length, end - Magic.Length);
            ValidateLabel(label);
            return label;
        }

        internal static byte[] Read(string configuredSave, int index)
        {
            using var stream = File.OpenRead(PathFor(configuredSave, index));
            ReadHeader(stream);
            var payload = new byte[checked((int)stream.Length - HeaderBytes)];
            var offset = 0;
            while (offset < payload.Length)
            {
                var count = stream.Read(payload, offset, payload.Length - offset);
                if (count == 0) throw new InvalidDataException("campaign slot replay is truncated");
                offset += count;
            }
            return payload;
        }

        internal static void Write(string configuredSave, int index, string label, byte[] replay)
        {
            ValidateLabel(label);
            if (replay == null || replay.Length == 0 || replay.Length > GameRuntimeTransport.MaximumReplaySaveJsonBytes)
                throw new InvalidDataException("campaign slot requires a bounded Rust replay payload");
            var path = PathFor(configuredSave, index);
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            var temporary = path + ".tmp." + Guid.NewGuid().ToString("N");
            try
            {
                using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
                {
                    var header = new byte[HeaderBytes];
                    Buffer.BlockCopy(Magic, 0, header, 0, Magic.Length);
                    Encoding.ASCII.GetBytes(label, 0, label.Length, header, Magic.Length);
                    stream.Write(header, 0, header.Length);
                    stream.Write(replay, 0, replay.Length);
                    stream.Flush(true);
                }
                if (File.Exists(path)) File.Replace(temporary, path, null);
                else File.Move(temporary, path);
            }
            finally { if (File.Exists(temporary)) File.Delete(temporary); }
        }

        private static void ValidateLabel(string label)
        {
            if (label == null || label.Length > 15) throw new InvalidDataException("campaign slot label exceeds 15 source characters");
            foreach (var value in label)
                if (value != ' ' && (value < 'A' || value > 'Z') && (value < '0' || value > '9'))
                    throw new InvalidDataException("campaign slot label must use source uppercase letters, digits or spaces");
        }
    }
}
