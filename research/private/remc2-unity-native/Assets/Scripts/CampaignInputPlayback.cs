using System;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Opt-in capture tooling: replay public controls at the ordinary input join.
    // It does not place actors, change state, or advance the native clock itself.
    internal sealed class CampaignInputPlayback
    {
        [Serializable] internal sealed class Frame
        {
            public string longitudinal, lateral, select_spell;
            public int roll_axis, pitch_axis, look_yaw, look_pitch;
            public bool cast_left, cast_right, cast_selected;
            public uint elapsed_centiseconds;
            internal sbyte Flags => (sbyte)((longitudinal == "accelerate" ? 1 : longitudinal == "decelerate" ? 2 : 0) |
                (lateral == "negative" ? 4 : lateral == "positive" ? 8 : 0) |
                (cast_left ? 16 : 0) | (cast_right ? 32 : 0) | (cast_selected ? 64 : 0));
        }
        [Serializable] private sealed class Stream
        {
            public string schema, startingTick, replaySha256;
            public Frame[] commands;
        }
        private static readonly Frame Neutral = new Frame { longitudinal = "coast", lateral = "neutral" };
        private readonly Stream stream;
        private readonly ulong start;
        private ulong? boundGeneration;

        internal CampaignInputPlayback(string[] arguments)
        {
            var path = Option(arguments, "-mc-input-playback");
            if (path == null) return;
            if (new FileInfo(path).Length > 2 * 1024 * 1024) throw new ArgumentException("Input playback exceeds 2 MiB");
            stream = JsonUtility.FromJson<Stream>(File.ReadAllText(path));
            if (stream == null || stream.schema != "mc-campaign-input-playback/v1" || stream.commands == null ||
                stream.commands.Length == 0 || stream.commands.Length > 3600 ||
                !ulong.TryParse(stream.startingTick, NumberStyles.None, CultureInfo.InvariantCulture, out start))
                throw new ArgumentException("Invalid bounded campaign input stream");
            var replay = Option(arguments, "-mc-runtime-replay-load");
            if (replay == null) throw new ArgumentException("Input playback requires its starting native replay");
            using (var sha = SHA256.Create())
            using (var file = File.OpenRead(replay))
                if (!string.Equals(BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant(),
                    stream.replaySha256, StringComparison.Ordinal))
                    throw new ArgumentException("Input playback starting replay hash differs");
            foreach (var frame in stream.commands)
            {
                if (frame == null || !(frame.longitudinal == "coast" || frame.longitudinal == "accelerate" || frame.longitudinal == "decelerate") ||
                    !(frame.lateral == "neutral" || frame.lateral == "negative" || frame.lateral == "positive") ||
                    frame.roll_axis < -128 || frame.roll_axis > 127 || frame.pitch_axis < -128 || frame.pitch_axis > 127 ||
                    frame.look_yaw < short.MinValue || frame.look_yaw > short.MaxValue || frame.look_pitch < short.MinValue || frame.look_pitch > short.MaxValue)
                    throw new ArgumentException("Input playback contains an invalid public control frame");
                // JsonUtility reads a JSON null string as empty. An absent
                // optional selection must remain no command at the native join.
                if (frame.select_spell == string.Empty) frame.select_spell = null;
            }
            Debug.Log($"MC_INPUT_PLAYBACK_READY start={start} commands={stream.commands.Length} native_controls_only=True");
        }

        internal Frame At(ulong generation, ulong tick)
        {
            if (stream == null) return null;
            if (!boundGeneration.HasValue)
            {
                if (tick != start) throw new InvalidOperationException("Input playback did not begin at its recorded tick");
                boundGeneration = generation;
            }
            if (generation != boundGeneration.Value || tick < start)
                throw new InvalidOperationException("Input playback owner changed; a new stream is required");
            var index = tick - start;
            return index < (ulong)stream.commands.Length ? stream.commands[(int)index] : Neutral;
        }

        private static string Option(string[] args, string option)
        {
            var index = Array.IndexOf(args, option);
            if (index < 0) return null;
            if (index + 1 >= args.Length || string.IsNullOrWhiteSpace(args[index + 1]))
                throw new ArgumentException(option + " requires a path");
            return args[index + 1];
        }
    }
}
