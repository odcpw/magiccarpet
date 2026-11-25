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
        internal bool IsAvailable => source != null && clips.Count == 27;
        internal int PlayCount { get; private set; }
        internal int LastTrack { get; private set; }
        internal uint LastStartMs { get; private set; }
        internal uint LastLengthMs { get; private set; }
        private int soundVolume = 127, musicVolume = 127;
        private int soundUnits = 127, musicUnits = 127;
        private bool ducked, restoring;
        private double speechEnd;
        private float restoreTicks;
        internal float SoundMix => soundVolume == 0 ? 0 : soundUnits / (float)soundVolume;
        internal float MusicMix => musicVolume == 0 ? 0 : musicUnits / (float)musicVolume;

        internal CampaignSpeechPresenter(Transform parent, string directory)
        {
            var catalog = JsonUtility.FromJson<Catalog>(
                File.ReadAllText(Path.Combine(directory, "speech.json")));
            if (catalog?.schema != "mc2-campaign-speech/v1" || catalog.sample_rate != 44100 ||
                catalog.channels != 2 || catalog.sample_width != 2 || catalog.tracks?.Length != 27)
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
                    if (track == null || track.track < 1 || track.track > 27 || clips.ContainsKey(track.track) ||
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
            var lengthSamples = checked(((long)lengthMs * clip.frequency + 999) / 1000);
            if (startSample >= clip.samples || startSample + lengthSamples > clip.samples)
                throw new InvalidDataException("Campaign speech segment extends beyond its track");
            source.Stop(); // the source owns one interruptible speech channel
            source.clip = clip;
            source.timeSamples = checked((int)startSample);
            source.Play();
            speechEnd = AudioSettings.dspTime + lengthMs / 1000.0;
            source.SetScheduledEndTime(speechEnd);
            PlayCount++;
            LastTrack = track;
            LastStartMs = startMs;
            LastLengthMs = lengthMs;
            Debug.Log($"MC_CAMPAIGN_SPEECH track={track} start_ms={startMs} length_ms={lengthMs}");
        }

        internal void ApplyPreferences(PlayerPreferences preferences)
        {
            // The options slider writes the selected master immediately. An
            // unchanged preference leaves that channel's speech fade intact.
            if (soundVolume != preferences.sound_volume) soundUnits = preferences.sound_volume;
            if (musicVolume != preferences.music_volume) musicUnits = preferences.music_volume;
            soundVolume = preferences.sound_volume;
            musicVolume = preferences.music_volume;
            if (!ducked && !restoring) { soundUnits = soundVolume; musicUnits = musicVolume; }
            // ChangeSettings_1A970's Speech option only gates future cues.
            // The current CD segment continues; the Sound option separately
            // owns StopCdPlayback and the device stop edge.
            source.volume = preferences.sound_enabled
                ? preferences.sound_volume / 127f : 0;
        }

        internal void Duck()
        {
            // FadeDownSoundVolume_59A50 cancels restoration and divides the
            // configured integer master volumes, not their floating gains.
            restoring = false;
            ducked = true;
            restoreTicks = 0;
            soundUnits = soundVolume / 3;
            musicUnits = musicVolume / 3;
        }

        internal void Tick(float elapsed)
        {
            if (ducked && AudioSettings.dspTime >= speechEnd && !source.isPlaying)
            {
                ducked = false;
                restoring = true;
                restoreTicks = 0;
                return;
            }
            if (!restoring) return;
            // FadeUpSoundVolume_59B50 runs at 120 Hz independently of pause.
            restoreTicks += elapsed * 120;
            var ticks = Mathf.FloorToInt(restoreTicks);
            restoreTicks -= ticks;
            soundUnits = Math.Min(soundVolume, soundUnits + ticks);
            musicUnits = Math.Min(musicVolume, musicUnits + ticks);
            if (soundUnits == soundVolume && musicUnits == musicVolume) restoring = false;
        }

        internal void Stop()
        {
            source.Stop(); source.clip = null;
            ducked = restoring = false;
            soundUnits = soundVolume; musicUnits = musicVolume;
            restoreTicks = 0;
        }
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
