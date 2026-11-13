using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    /// <summary>Device/cache adapter for the instance-owned Rust media clock.</summary>
    internal sealed class FrontendMediaPresenter : IDisposable
    {
        [Serializable] private sealed class Catalog
        {
            public string schema;
            public Output[] outputs;
            public Glyph[] glyphs;
            public AudioBinding[] audio;
            public CreditsArt credits_art;
        }
        [Serializable] private sealed class CreditsArt
        {
            public string frames_path, font_path;
            public int width, height, font_width, font_height, advance_x, advance_y;
            public CreditsGlyph[] glyphs;
            [NonSerialized] public byte[] mask;
        }
        [Serializable] private sealed class CreditsGlyph
        { public int index, width, height; }
        [Serializable] private sealed class AudioBinding
        { public string kind, path; public int bank, id; }
        [Serializable] private sealed class Output
        {
            public string path;
            public long bytes;
            public string sha256;
        }
        [Serializable] private sealed class Glyph
        {
            public int index, width, height;
            public string path;
            [NonSerialized] public byte[] mask;
        }
        private readonly string directory;
        private readonly Transform parent;
        private readonly bool startup;
        private readonly Dictionary<string, AudioClip> clips = new();
        private readonly Dictionary<int, AudioSource> samples = new();
        private readonly Dictionary<int, int> sampleUnits = new();
        private readonly HashSet<int> speechSamples = new();
        private float soundGain = 1, musicGain = 1, musicUnits = 127;
        private bool speechEnabled = true;
        private Catalog catalog;
        private string catalogJson;
        private GameObject audioRoot;
        private AudioSource music;
        private FileStream frames;
        private Texture2D texture;
        private Texture2D creditsTexture;
        private byte[] creditsPixels, creditsPalette;
        private int creditsCursor = -1;
        private int creditsFadeIn = -1, creditsFadeOut = -1;
        private byte[] pixels, palette;
        private Color32[] colors;
        private uint epoch;
        private int frameIndex = -1;
        private int fadeIn = -1, fadeOut = -1;
        private bool installed, installAttempted, campaignInstallAttempted, mapMusicFailed;
        private GameRuntimeTransport.MediaSnapshot current;
        internal bool Visible => current?.active == true || current?.campaign_return?.blocking == true;
        internal string Error { get; private set; }

        internal FrontendMediaPresenter(Transform parent, string directory, bool startup = true)
        {
            this.parent = parent;
            this.directory = Path.GetFullPath(directory);
            this.startup = startup;
        }

        internal bool Step(ulong handle, GameRuntimeTransport.Identity identity,
            uint elapsedUs, bool input, bool escape)
        {
            var wasVisible = Visible;
            try
            {
                var campaignReturn = identity.frontend_media?.campaign_return?.blocking == true;
                if (!installed && (!installAttempted || campaignReturn && !campaignInstallAttempted))
                {
                    installAttempted = true;
                    if (campaignReturn) campaignInstallAttempted = true;
                    LoadCatalog();
                    Send(handle, identity, "{\"operation\":\"install\",\"catalog\":" + catalogJson +
                        ",\"startup\":" + (startup ? "true" : "false") +
                        ",\"audio_available\":" + (SystemInfo.supportsAudio ? "true" : "false") + "}", true);
                    Error = null;
                    mapMusicFailed = false;
                    return true;
                }
                Send(handle, identity, "{\"operation\":\"frame\",\"elapsed_us\":" + elapsedUs +
                    ",\"input\":" + (input ? "true" : "false") +
                    ",\"escape\":" + (escape ? "true" : "false") + "}");
                return wasVisible || Visible || current?.clear_input == true;
            }
            catch (Exception exception)
            {
                Error = exception.Message;
                Debug.LogError("MC_FRONTEND_MEDIA_ERROR " + Error);
                ReleaseSegment();
                StopAudio();
                // JsonUtility supplies correct JSON string escaping, including
                // paths and decoder diagnostics, through a typed request.
                try
                {
                    GameRuntimeTransport.MediaCommand(handle, identity,
                        JsonUtility.ToJson(new Failure { message = Error }));
                    // A Title failure clears Rust's catalog; a campaign-return
                    // failure retains it while draining the held input.
                    if (identity.frontend_media?.campaign_return?.blocking != true)
                        installed = false;
                    current = GameRuntimeTransport.ReadIdentity(handle).frontend_media;
                }
                catch (Exception failure)
                {
                    Error += "; failed to report media error: " + failure.Message;
                    Debug.LogError("MC_FRONTEND_MEDIA_ERROR " + Error);
                }
                return true;
            }
        }
        [Serializable] private sealed class Failure
        {
            public string operation = "failed";
            public string message;
        }

        private void LoadCatalog()
        {
            if (catalogJson != null) return;
            var json = File.ReadAllText(PathFor("media.json"));
            var candidate = JsonUtility.FromJson<Catalog>(json);
            if (candidate?.schema != "mc2-frontend-media/v1" || candidate.outputs == null || candidate.glyphs == null)
                throw new InvalidDataException("invalid frontend media manifest: " + PathFor("media.json"));
            // The same receipt gate is used by cold Title and restored map.
            foreach (var output in candidate.outputs)
            {
                using var stream = File.OpenRead(PathFor(output.path));
                using var sha = SHA256.Create();
                var hash = BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "").ToLowerInvariant();
                if (stream.Length != output.bytes || hash != output.sha256)
                    throw new InvalidDataException("frontend media extent/hash mismatch: " + PathFor(output.path));
            }
            foreach (var glyph in candidate.glyphs)
            {
                glyph.mask = File.ReadAllBytes(PathFor(glyph.path));
                if (glyph.mask.Length != checked(glyph.width * glyph.height))
                    throw new InvalidDataException("frontend subtitle glyph extent: " + glyph.path);
            }
            if (!candidate.glyphs.Any(g => g.index == 65 && g.width > 0 && g.height > 0))
                throw new InvalidDataException("frontend subtitle font is missing glyph 65: " + PathFor("media.json"));
            if (candidate.credits_art != null)
            {
                var art = candidate.credits_art;
                if (art.width != 640 || art.height != 480 || art.font_width != 160 || art.font_height != 252 ||
                    art.advance_x != 8 || art.advance_y != 14 || art.glyphs?.Length != 273 ||
                    !candidate.outputs.Any(row => row.path == art.frames_path) ||
                    !candidate.outputs.Any(row => row.path == art.font_path))
                    throw new InvalidDataException("invalid source credits art bindings");
                art.mask = File.ReadAllBytes(PathFor(art.font_path));
                if (art.mask.Length != checked(art.font_width * art.font_height))
                    throw new InvalidDataException("source credits font mask extent");
                for (var i = 0; i < art.glyphs.Length; i++)
                {
                    var glyph = art.glyphs[i];
                    if (glyph == null || glyph.index != i || glyph.width < 0 || glyph.width > 10 ||
                        glyph.height < 0 || glyph.height > 14 || (glyph.width == 0) != (glyph.height == 0))
                        throw new InvalidDataException("source credits glyph extent");
                }
            }
            catalog = candidate;
            catalogJson = json;
        }

        /// MainMenu starts track 4 before NewGameDialog, including a restored
        /// map with no preceding Title/media clock in this process.
        internal void StartMapMusic()
        {
            if (mapMusicFailed) return;
            try
            {
                LoadCatalog();
                var binding = catalog.audio?.SingleOrDefault(row => row.kind == "music" && row.bank == 0 && row.id == 4)
                    ?? throw new InvalidDataException("source menu music bank0/track4 is missing");
                if (!catalog.outputs.Any(row => row.path == binding.path))
                    throw new InvalidDataException("source menu music has no validated output receipt");
                if (music != null && music.isPlaying && clips.TryGetValue(binding.path, out var clip) && music.clip == clip) return;
                ApplyAudio(new GameRuntimeTransport.MediaAction { op = "M", value = 4, asset_path = binding.path });
            }
            catch (Exception exception)
            {
                mapMusicFailed = true;
                Error = exception.Message;
                StopAudio();
                Debug.LogError("MC_FRONTEND_MEDIA_ERROR " + Error);
            }
        }
        private void Send(ulong handle, GameRuntimeTransport.Identity identity, string json, bool installing = false)
        {
            var previousActive = current?.active == true;
            GameRuntimeTransport.MediaCommand(handle, identity, json);
            // Record native installation before observing or rendering it: a
            // subsequent decoder failure does not undo the native command.
            if (installing) installed = true;
            current = GameRuntimeTransport.ReadIdentity(handle).frontend_media;
            if (current == null || current.contract != "mc2-frontend-media/v1")
                throw new InvalidDataException("native frontend media observation is unavailable");
            if (current.campaign_return == null) campaignInstallAttempted = false;
            if (current.epoch != epoch)
            {
                ReleaseSegment(current.retain_previous_frame);
                epoch = current.epoch;
                Debug.Log($"MC_FRONTEND_MEDIA_BEGIN epoch={epoch} clip={current.clip_id}");
            }
            if (!current.active && previousActive)
            {
                ReleaseSegment();
                StopAudio();
                Debug.Log("MC_FRONTEND_MEDIA_TITLE audio=menu input=cleared");
            }
            foreach (var action in current.actions ?? Array.Empty<GameRuntimeTransport.MediaAction>())
                ApplyAudio(action);
            if (current.campaign_return?.phase == "credits")
            {
                ComposeCredits(current.campaign_return.credits);
                return;
            }
            if (!current.active)
            {
                if (frames != null || texture != null)
                {
                    ReleaseSegment();
                    Debug.Log("MC_FRONTEND_MEDIA_TITLE audio=menu input=cleared");
                }
                return;
            }
            if (current.frame_index < 0 || frameIndex == current.frame_index &&
                fadeIn == current.fade_in_level && fadeOut == current.fade_out_level) return;
            if (frames == null)
            {
                frames = File.OpenRead(PathFor(current.frames_path));
                pixels = new byte[checked(current.width * current.height)];
                palette = new byte[768];
                colors = new Color32[pixels.Length];
                ReleaseObject(texture);
                texture = new Texture2D(current.width, current.height, TextureFormat.RGBA32, false)
                { name = "MC2 owner FMV " + current.clip_id, filterMode = FilterMode.Point };
            }
            var stride = checked(pixels.Length + 768);
            frames.Position = checked((long)stride * current.frame_index);
            ReadExactly(frames, pixels);
            ReadExactly(frames, palette);
            ComposeIndexedFrame();
            texture.SetPixels32(colors);
            texture.Apply(false, false);
            var frameChanged = frameIndex != current.frame_index;
            frameIndex = current.frame_index;
            fadeIn = current.fade_in_level;
            fadeOut = current.fade_out_level;
            if (frameChanged && (frameIndex == 0 || frameIndex % 200 == 0))
                Debug.Log($"MC_FRONTEND_MEDIA_FRAME clip={current.clip_id} frame={frameIndex} subtitle={current.subtitle}");
        }
        private void ComposeCredits(GameRuntimeTransport.CreditsSnapshot view)
        {
            var art = catalog?.credits_art ?? throw new InvalidDataException("source credits art is missing");
            if (view?.lines == null || view.cursor < 0 || view.cursor > 117 || view.lines.Length > 8)
                throw new InvalidDataException("invalid Rust credits page");
            if (creditsTexture != null && creditsCursor == view.cursor &&
                creditsFadeIn == current.fade_in_level && creditsFadeOut == current.fade_out_level) return;
            if (creditsPixels == null)
            {
                using var stream = File.OpenRead(PathFor(art.frames_path));
                creditsPixels = new byte[checked(art.width * art.height)];
                creditsPalette = new byte[768];
                ReadExactly(stream, creditsPixels);
                ReadExactly(stream, creditsPalette);
                if (stream.Position != stream.Length)
                    throw new InvalidDataException("source credits background extent");
            }
            var image = (byte[])creditsPixels.Clone();
            var white = 0;
            var distance = int.MaxValue;
            for (var i = 0; i < 256; i++)
            {
                var d = 0;
                for (var c = 0; c < 3; c++) { var delta = 63 - creditsPalette[i * 3 + c]; d += delta * delta; }
                if (d < distance) { distance = d; white = i; }
            }
            for (var line = 0; line < view.lines.Length; line++)
            {
                var text = view.lines[line] ?? throw new InvalidDataException("null source credits line");
                if (text.Length > 33 || text.Any(ch => ch > 255))
                    throw new InvalidDataException("invalid source credits line");
                // DrawText_7FAE0 centres between x10 and x620 and omits a
                // one-character string. Bank6's font pointer skips TAB row0.
                if (text.Length == 1) continue;
                var x = 315 - art.advance_x * text.Length / 2;
                var y = 200 + line * (art.advance_y + 2);
                foreach (var ch in text)
                {
                    var glyph = art.glyphs[ch + 1];
                    if (ch != ' ' && ch != '\t')
                    {
                        var gx = glyph.index % 16 * 10;
                        var gy = glyph.index / 16 * 14;
                        for (var dy = 0; dy < glyph.height; dy++)
                            for (var dx = 0; dx < glyph.width; dx++)
                                if (art.mask[(gy + dy) * art.font_width + gx + dx] != 0 &&
                                    x + dx >= 0 && x + dx < art.width && y + dy < art.height)
                                    image[(y + dy) * art.width + x + dx] = (byte)white;
                    }
                    x += art.advance_x;
                }
            }
            var rgba = new Color32[image.Length];
            for (var y = 0; y < art.height; y++)
                for (var x = 0; x < art.width; x++)
                {
                    var i = image[y * art.width + x] * 3;
                    rgba[(art.height - 1 - y) * art.width + x] = new Color32(
                        Channel(creditsPalette[i]), Channel(creditsPalette[i + 1]),
                        Channel(creditsPalette[i + 2]), 255);
                }
            if (creditsTexture == null)
                creditsTexture = new Texture2D(art.width, art.height, TextureFormat.RGBA32, false)
                { name = "MC2 source credits", filterMode = FilterMode.Point };
            creditsTexture.SetPixels32(rgba);
            creditsTexture.Apply(false, false);
            creditsCursor = view.cursor;
            creditsFadeIn = current.fade_in_level;
            creditsFadeOut = current.fade_out_level;
        }
        private void ComposeIndexedFrame()
        {
            var image = new byte[pixels.Length];
            var offset = current.subtitle_viewport_offset;
            if (offset < 0 || offset > pixels.Length) throw new InvalidDataException("invalid media subtitle viewport");
            Buffer.BlockCopy(pixels, offset, image, 0, pixels.Length - offset);
            // Palette changes reselect source nearest-white and redraw retained
            // subtitle text, even when the text event did not change this frame.
            var white = 0;
            var distance = 9999;
            for (var i = 0; i < 256; i++)
            {
                var d = 0;
                for (var c = 0; c < 3; c++) { var delta = 63 - palette[i * 3 + c]; d += delta * delta; }
                if (d < distance) { distance = d; white = i; }
            }
            if (offset != 0 && !string.IsNullOrEmpty(current.subtitle))
                DrawSubtitle(image, current.subtitle, (byte)white);
            for (var y = 0; y < current.height; y++)
                for (var x = 0; x < current.width; x++)
                {
                    var i = image[y * current.width + x] * 3;
                    colors[(current.height - 1 - y) * current.width + x] = new Color32(
                        Channel(palette[i]), Channel(palette[i + 1]), Channel(palette[i + 2]), 255);
                }
        }
        private byte Channel(byte channel)
        {
            var value = channel * current.fade_in_level / 32;
            value += -value * current.fade_out_level / 16;
            return unchecked((byte)(value * 4));
        }
        private void DrawSubtitle(byte[] image, string text, byte white)
        {
            var cell = catalog.glyphs.First(g => g.index == 65);
            // The source power constructor doubles table dimensions, then
            // low-resolution drawing halves them. Cache dimensions are final.
            var advance = Math.Max(1, cell.width);
            var lineHeight = Math.Max(1, cell.height);
            var capacity = Math.Max(1, 315 / advance - 3);
            var lines = new List<string>();
            var line = "";
            foreach (var word in text.Split(' '))
            {
                if (line.Length > 0 && line.Length + 1 + word.Length > capacity)
                { lines.Add(line); line = ""; }
                line += (line.Length == 0 ? "" : " ") + word;
            }
            if (line.Length > 0) lines.Add(line);
            var y = 170;
            foreach (var row in lines)
            {
                var x = (315 - advance * row.Length) / 2;
                foreach (var ch in row)
                {
                    var glyph = catalog.glyphs.FirstOrDefault(g => g.index == ch);
                    if (glyph != null)
                        for (var gy = 0; gy < glyph.height; gy++)
                            for (var gx = 0; gx < glyph.width; gx++)
                                if (glyph.mask[gy * glyph.width + gx] != 0 &&
                                    x + gx >= 0 && x + gx < current.width && y + gy < current.height)
                                    image[(y + gy) * current.width + x + gx] = white;
                    x += advance;
                }
                y += lineHeight;
            }
        }
        private void ApplyAudio(GameRuntimeTransport.MediaAction action)
        {
            EnsureAudioRoot();
            var op = action.op?.ToUpperInvariant();
            if (op == "X" || op == "B") { music.Stop(); music.clip = null; return; }
            if (op == "E" || op == "T" && action.value == 0)
            { foreach (var source in samples.Values) source.Stop(); return; }
            if (op == "G")
            {
                sampleUnits[action.value] = action.volume_units ??
                    throw new InvalidDataException("source gain action has no volume");
                if (samples.TryGetValue(action.value, out var gainSource))
                    gainSource.volume = SampleGain(action.value);
                return;
            }
            // Source timer callbacks, scheduled by Rust, apply sample ramps.
            if (op == "F" || op == "O" || op == "P") return;
            if (op == "T")
            {
                if (samples.TryGetValue(action.value, out var source))
                {
                    source.Stop();
                }
                return;
            }
            var musicAction = op == "D" || op == "L" || op == "M" || op == "Z";
            if (!musicAction && op != "S" && op != "R" && op != "H") return;
            if (op == "S" && action.value == 0)
            { foreach (var source in samples.Values) source.Stop(); return; }
            if (string.IsNullOrEmpty(action.asset_path))
                throw new InvalidDataException($"missing owner media audio binding {op}/{action.value}");
            if (!clips.TryGetValue(action.asset_path, out var clip))
            {
                clip = OwnerAudioPresentation.CreateMediaClip(action.asset_path,
                    File.ReadAllBytes(PathFor(action.asset_path)));
                clips.Add(action.asset_path, clip);
            }
            if (musicAction)
            {
                music.Stop(); music.clip = clip; music.loop = true;
                musicUnits = op == "D" ? 100 : 127;
                music.volume = musicUnits / 127f * musicGain;
                music.Play();
            }
            else
            {
                if (!samples.TryGetValue(action.value, out var source))
                {
                    source = audioRoot.AddComponent<AudioSource>();
                    source.playOnAwake = false; source.spatialBlend = 0;
                    samples.Add(action.value, source);
                }
                if (source.isPlaying) return;
                source.clip = clip; source.loop = op != "S";
                if (action.value is 3 or 4 or 5 or 6 or 7 or 8 or 15) speechSamples.Add(action.value);
                else speechSamples.Remove(action.value);
                sampleUnits[action.value] = op == "H" ? 0 : 127;
                source.volume = SampleGain(action.value);
                source.Play();
            }
            Debug.Log($"MC_FRONTEND_MEDIA_AUDIO op={op} id={action.value} asset={action.asset_path}");
        }

        private void EnsureAudioRoot()
        {
            if (audioRoot != null) return;
            audioRoot = new GameObject("MC2 frontend media audio");
            audioRoot.transform.SetParent(parent, false);
            music = audioRoot.AddComponent<AudioSource>();
            music.playOnAwake = false;
            music.spatialBlend = 0;
        }

        /// Menu samples share the media device/lifetime, so entering the map
        /// keeps track 4 playing and the real loading boundary stops both.
        internal void PlayMenuSample(int id, byte[] wave, int volume, int pan, int rate, bool ifNotPlaying, int loops = 0)
        {
            if (id <= 0 || volume < 0 || volume > 127 || pan < 0 || pan > 127 || rate <= 0 || rate > 400 ||
                loops < -1 || loops > 0)
                throw new ArgumentOutOfRangeException(nameof(id), "invalid source menu sample command");
            if (ifNotPlaying && samples.TryGetValue(id, out var active) && active.isPlaying) return;
            EnsureAudioRoot();
            using var sha = SHA256.Create();
            var key = "menu/" + id + "/" + BitConverter.ToString(sha.ComputeHash(wave));
            if (!clips.TryGetValue(key, out var clip))
            {
                clip = OwnerAudioPresentation.CreateMediaClip("MC2 menu sample " + id, wave);
                clips.Add(key, clip);
            }
            if (!samples.TryGetValue(id, out var source))
            {
                source = audioRoot.AddComponent<AudioSource>();
                source.playOnAwake = false;
                source.spatialBlend = 0;
                samples.Add(id, source);
            }
            source.Stop();
            source.clip = clip;
            // PlaySample_8F100 map/menu calls are sound effects, even when a
            // sample number also names speech in the intro's media catalogue.
            speechSamples.Remove(id);
            source.loop = loops == -1;
            source.panStereo = (pan - 64) / 64f;
            source.pitch = rate / 100f;
            sampleUnits[id] = volume;
            source.volume = SampleGain(id);
            source.Play();
            Debug.Log($"MC_FRONTEND_MENU_SAMPLE id={id} volume={volume} pan={pan} rate={rate}");
        }
        private float SampleGain(int id)
        {
            var speech = speechSamples.Contains(id);
            return speech && !speechEnabled ? 0 : sampleUnits.GetValueOrDefault(id, 127) / 127f * soundGain;
        }
        internal void ApplyPreferences(PlayerPreferences preferences)
        {
            soundGain = preferences.sound_enabled ? preferences.sound_volume / 127f : 0;
            musicGain = preferences.music_enabled ? preferences.music_volume / 127f : 0;
            speechEnabled = preferences.speech_enabled;
            if (music != null) music.volume = musicUnits / 127f * musicGain;
            foreach (var pair in samples) pair.Value.volume = SampleGain(pair.Key);
        }
        internal void Draw()
        {
            if (!Visible) return;
            GUI.depth = -1000;
            GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), Texture2D.blackTexture);
            if (current.campaign_return?.phase == "credits")
            {
                if (creditsTexture != null)
                    GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), creditsTexture, ScaleMode.ScaleToFit);
                return;
            }
            if (current.campaign_return?.phase == "input_release") return;
            if (texture != null && (current.frame_index >= 0 || current.retain_previous_frame))
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), texture, ScaleMode.ScaleToFit);
        }
        private string PathFor(string relative)
        {
            if (string.IsNullOrEmpty(relative) || Path.IsPathRooted(relative) || relative.Contains('\\') ||
                relative.Split('/').Any(p => p == ".." || p == "." || p.Length == 0))
                throw new InvalidDataException("invalid frontend media cache path: " + relative);
            return Path.Combine(directory, relative);
        }
        private static void ReadExactly(Stream stream, byte[] data)
        {
            var offset = 0;
            while (offset < data.Length)
            {
                var n = stream.Read(data, offset, data.Length - offset);
                if (n == 0) throw new InvalidDataException("truncated media frame at byte " + stream.Position);
                offset += n;
            }
        }
        private void ReleaseSegment(bool retainTexture = false)
        {
            frames?.Dispose(); frames = null;
            if (!retainTexture)
            {
                ReleaseObject(texture);
                texture = null;
            }
            pixels = null; palette = null; colors = null; frameIndex = -1;
        }
        internal void EndMenuSamples()
        {
            foreach (var source in samples.Values) { source.Stop(); ReleaseObject(source); }
            samples.Clear();
            sampleUnits.Clear();
            speechSamples.Clear();
        }
        internal void StopAudio()
        {
            // The outer runtime owns the listener across frontend/gameplay.
            EndMenuSamples();
            if (music != null) { music.Stop(); music.clip = null; }
            foreach (var clip in clips.Values) ReleaseObject(clip);
            clips.Clear();
        }
        public void Dispose()
        {
            ReleaseSegment(); StopAudio();
            ReleaseObject(creditsTexture);
            creditsTexture = null; creditsPixels = null; creditsPalette = null;
            ReleaseObject(audioRoot);
            audioRoot = null; music = null; current = null;
        }
        private static void ReleaseObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
