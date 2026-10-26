using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using UnityEngine;

namespace MagicCarpet.Native
{
    // GameUI's player overlay calls DrawText_2BC10 with the player's palette
    // colour. FONT1 is a coverage mask, not a modern substitute font.
    internal sealed class SourceMinimapLabels
    {
        [Serializable] internal sealed class Label
        {
            public int source_order, subject_entity_index, x, y, palette_index;
            public string text;
        }

        [Serializable] internal sealed class Snapshot
        {
            public string schema;
            public ulong generation, tick;
            public Label[] labels;
        }

        internal sealed class Glyph
        {
            internal int Width, Height;
            internal byte[] Mask;
        }

        private readonly Dictionary<int, Glyph> glyphs = new();
        private OwnerAssetCache glyphOwner;

        internal void Clear()
        {
            if (Coverage != null) Array.Clear(coverage, 0, coverage.Length);
            Coverage = null;
        }
        private readonly bool[] coverage = new bool[320 * 200];
        internal bool[] Coverage { get; private set; }

        internal int Materialize(byte[] frame, NativeCampaignSession session,
            OwnerAssetCache cache, byte[] tables, ulong generation, ulong tick)
        {
            if (Coverage != null) Array.Clear(coverage, 0, coverage.Length);
            Coverage = null;
            var snapshot = Read(session.Handle, generation, tick);
            if (snapshot.labels.Length == 0) return 0;
            if (cache?.EnvironmentMaterials == null)
                throw new InvalidOperationException("Beyond Sight labels require the package FONT1 bank");
            if (!ReferenceEquals(glyphOwner, cache))
            {
                glyphs.Clear();
                glyphOwner = cache;
            }
            var font = cache.EnvironmentMaterials.bitmap_banks.Single(bank => bank.bank == "FONT1");
            foreach (var label in snapshot.labels)
            foreach (var character in label.text)
            {
                var selector = character + 1;
                if (glyphs.ContainsKey(selector)) continue;
                var row = font.bitmaps.Single(bitmap => bitmap.selector == selector);
                var mask = cache.ReadOutput(row.mask_path);
                if (row.source_empty || row.width <= 0 || row.height <= 0 ||
                    mask.Length != checked(row.width * row.height))
                    throw new InvalidOperationException("Beyond Sight FONT1 glyph is incomplete");
                glyphs.Add(selector, new Glyph { Width = row.width, Height = row.height, Mask = mask });
            }
            var written = Rasterize(frame, snapshot, tables, glyphs, generation, tick, coverage);
            if (written != 0) Coverage = coverage;
            return written;
        }

        private static Snapshot Read(ulong handle, ulong generation, ulong tick)
        {
            var status = NativeSim.mc_native_campaign_minimap_labels_json(
                handle, generation, tick, IntPtr.Zero, 0, out var required);
            if (status != NativeSim.Bounds || required == 0 || required > 8192)
                throw new InvalidOperationException("Beyond Sight label sizing failed: " + NativeSession.ReadLastError());
            var buffer = Marshal.AllocHGlobal(checked((int)required));
            try
            {
                status = NativeSim.mc_native_campaign_minimap_labels_json(
                    handle, generation, tick, buffer, required, out var written);
                if (status != NativeSim.Ok || written != required)
                    throw new InvalidOperationException("Beyond Sight label copy failed: " + NativeSession.ReadLastError());
                var bytes = new byte[checked((int)written)];
                Marshal.Copy(buffer, bytes, 0, bytes.Length);
                var snapshot = JsonUtility.FromJson<Snapshot>(Encoding.UTF8.GetString(bytes));
                if (snapshot == null || snapshot.schema != "mc2-campaign-minimap-labels/v1" ||
                    snapshot.generation != generation || snapshot.tick != tick || snapshot.labels == null)
                    throw new InvalidOperationException("Beyond Sight labels are from another revision");
                return snapshot;
            }
            finally { Marshal.FreeHGlobal(buffer); }
        }

        internal static int Rasterize(byte[] frame, Snapshot snapshot, byte[] tables,
            IReadOnlyDictionary<int, Glyph> font, ulong generation, ulong tick, bool[] coverage = null)
        {
            if (frame == null || frame.Length != 320 * 200 || tables == null || tables.Length != 83456 ||
                snapshot == null || snapshot.generation != generation || snapshot.tick != tick || snapshot.labels == null ||
                coverage != null && coverage.Length != frame.Length)
                throw new InvalidOperationException("Beyond Sight label raster inputs are invalid");
            var written = 0;
            var previous = -1;
            foreach (var label in snapshot.labels)
            {
                if (label == null || label.source_order <= previous || label.palette_index < 0 ||
                    label.palette_index > 255 || string.IsNullOrEmpty(label.text))
                    throw new InvalidOperationException("Beyond Sight label ordering or content is invalid");
                previous = label.source_order;
                // Source checks the dot against the circular map before drawing
                // its name. The text itself clips to the screen, not the circle.
                if (label.x < 0 || label.x >= 128 || label.y < 0 || label.y >= 128) continue;
                var phase = label.y < 64 ? 63 - label.y : label.y - 64;
                var halfSpan = (64 * tables[0x14300 + phase * 4]) >> 8;
                if (label.x < 64 - halfSpan || label.x >= 64 + halfSpan) continue;
                // The existing indexed frame is 320x200; map commands use
                // 128-unit source coordinates. FONT1 masks already have the
                // low-resolution dimensions (e.g. the four-by-seven N glyph).
                var cursor = (label.x + 4) / 2;
                var top = label.y / 2;
                foreach (var character in label.text)
                {
                    if (character > 255 || !font.TryGetValue(character + 1, out var glyph) ||
                        glyph.Width <= 0 || glyph.Height <= 0 || glyph.Mask.Length != checked(glyph.Width * glyph.Height))
                        throw new InvalidOperationException("Beyond Sight label has no source FONT1 glyph");
                    if (cursor >= 320) break;
                    for (var y = 0; y < glyph.Height; y++)
                    for (var x = 0; x < glyph.Width; x++)
                    {
                        var targetX = cursor + x;
                        var targetY = top + y;
                        if (targetX < 0 || targetX >= 320 || targetY < 0 || targetY >= 200 ||
                            glyph.Mask[y * glyph.Width + x] == 0) continue;
                        frame[targetY * 320 + targetX] = (byte)label.palette_index;
                        if (coverage != null) coverage[targetY * 320 + targetX] = true;
                        written++;
                    }
                    cursor += glyph.Width;
                }
            }
            return written;
        }
    }
}
