using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Device playback only. Rust owns objective phases and segment selection.
    internal sealed class CampaignSpeechPresenter : IDisposable
    {
        [Serializable] private sealed class Catalog
        {
            public string schema;
            public int sample_rate, channels, sample_width;
            public Track[] tracks;
        }
        [Serializable] private sealed class Track
        {
            public int track, frames;
            public long bytes;
            public string path, sha256;
        }
        private readonly Dictionary<int, AudioClip> clips = new();
        private readonly GameObject root;
        private readonly AudioSource source;
        internal bool IsPlaying => source.isPlaying;

        internal CampaignSpeechPresenter(Transform parent, string directory)
        {
            var catalog = JsonUtility.FromJson<Catalog>(
                File.ReadAllText(Path.Combine(directory, "speech.json")));
            if (catalog?.schema != "mc2-campaign-speech/v1" || catalog.sample_rate != 44100 ||
                catalog.channels != 2 || catalog.sample_width != 2 || catalog.tracks?.Length != 3)
                throw new InvalidDataException("Invalid campaign speech catalog");
            root = new GameObject("MC2 campaign speech");
            root.transform.SetParent(parent, false);
            source = root.AddComponent<AudioSource>();
            source.playOnAwake = false;
            source.spatialBlend = 0;
            try
            {
                foreach (var track in catalog.tracks)
                {
                    if (track.track < 1 || track.track > 3 || clips.ContainsKey(track.track) ||
                        track.path != $"TRACK{track.track:00}.wav" || track.frames <= 0)
                        throw new InvalidDataException("Invalid campaign speech track binding");
                    var bytes = File.ReadAllBytes(Path.Combine(directory, track.path));
                    using var sha = SHA256.Create();
                    var hash = BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
                    if (bytes.LongLength != track.bytes || hash != track.sha256)
                        throw new InvalidDataException("Campaign speech content changed: " + track.path);
                    var clip = OwnerAudioPresentation.CreateMediaClip(track.path, bytes);
                    clips.Add(track.track, clip);
                    if (clip.samples != track.frames || clip.frequency != 44100 || clip.channels != 2)
                        throw new InvalidDataException("Campaign speech PCM metadata changed: " + track.path);
                }
            }
            catch { Dispose(); throw; }
        }

        internal void Play(int track, uint startMs, uint lengthMs)
        {
            if (!clips.TryGetValue(track, out var clip) || lengthMs == 0)
                throw new InvalidDataException("Unbound campaign speech segment");
            // PlayCdTrackSegment in port_sdl_sound.cpp divides integer milliseconds
            // by 1000 before assigning double startPosSec. Preserve that source seek.
            var startSample = checked((long)(startMs / 1000) * clip.frequency);
            if (startSample >= clip.samples)
                throw new InvalidDataException("Campaign speech segment starts beyond its track");
            source.Stop(); // the source owns one interruptible speech channel
            source.clip = clip;
            source.timeSamples = checked((int)startSample);
            source.Play();
            source.SetScheduledEndTime(AudioSettings.dspTime + lengthMs / 1000.0);
            Debug.Log($"MC_CAMPAIGN_SPEECH track={track} start_ms={startMs} length_ms={lengthMs}");
        }

        internal void ApplyPreferences(PlayerPreferences preferences)
        {
            source.volume = preferences.speech_enabled && preferences.sound_enabled
                ? preferences.sound_volume / 127f : 0;
        }

        internal void Stop() { source.Stop(); source.clip = null; }
        public void Dispose()
        {
            if (source != null) source.Stop();
            foreach (var clip in clips.Values) Release(clip);
            clips.Clear();
            Release(root);
        }
        private static void Release(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
