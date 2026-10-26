using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SourceMinimapLabelsTests
    {
        [Test]
        public void OriginalFontMasksPreserveCoverageAndLabelsBeyondMapEdge()
        {
            var root = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../generated/mc-level2-vault-successor-environment-20261025"));
            var descriptor = JsonUtility.FromJson<OwnerAssetCache.EnvironmentDescriptor>(
                File.ReadAllText(Path.Combine(root, "materials/descriptor.json")));
            var bank = descriptor.bitmap_banks.Single(row => row.bank == "FONT1");
            var font = new Dictionary<int, SourceMinimapLabels.Glyph>();
            foreach (var character in "Nyphur")
            {
                var row = bank.bitmaps.Single(glyph => glyph.selector == character + 1);
                font[character + 1] = new SourceMinimapLabels.Glyph {
                    Width = row.width, Height = row.height,
                    Mask = File.ReadAllBytes(Path.Combine(root, row.mask_path))
                };
            }
            var tables = File.ReadAllBytes(Path.Combine(root, descriptor.tables.path));
            var frame = Enumerable.Repeat((byte)3, 320 * 200).ToArray();
            var coverage = new bool[frame.Length];
            var snapshot = new SourceMinimapLabels.Snapshot {
                generation = 7, tick = 11,
                labels = new[] { new SourceMinimapLabels.Label {
                    source_order = 1001, subject_entity_index = 112,
                    x = 120, y = 64, palette_index = 92, text = "Nyphur"
                } }
            };
            var written = SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 11, coverage);
            Assert.That(written, Is.EqualTo("Nyphur".Sum(character =>
                font[character + 1].Mask.Count(value => value != 0))));
            Assert.That(coverage.Where((value, index) => value && index % 320 >= 64).Any(), Is.True,
                "source name must survive outside the minimap rectangle");
            Assert.That(frame.Where((value, index) => coverage[index]).All(value => value == 92), Is.True);
            Assert.That(frame.Where((value, index) => !coverage[index]).All(value => value == 3), Is.True,
                "transparent FONT1 pixels leave the indexed frame unchanged");

            snapshot.labels[0].x = 0;
            snapshot.labels[0].y = 0;
            Assert.That(SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 11), Is.Zero,
                "off-circle dot suppresses its entire label");
            Assert.Throws<InvalidOperationException>(() =>
                SourceMinimapLabels.Rasterize(frame, snapshot, tables, font, 7, 12));
        }

        [Test]
        public void OverlayRetainsOnlyNameCoverageOutsideMapAndRestoresSmallTexture()
        {
            var gameObject = new GameObject("Source minimap label regression");
            try
            {
                var overlay = gameObject.AddComponent<CampaignMinimapOverlay>();
                var frame = new byte[320 * 200];
                var palette = new byte[768];
                var tables = new byte[83456];
                var coverage = new bool[frame.Length];
                frame[32 * 320 + 90] = 7;
                palette[21] = 210;
                coverage[32 * 320 + 90] = true;
                overlay.Present(frame, palette, tables, coverage);
                var field = typeof(CampaignMinimapOverlay).GetField("texture",
                    BindingFlags.NonPublic | BindingFlags.Instance);
                var texture = (Texture2D)field.GetValue(overlay);
                Assert.That(texture.width, Is.EqualTo(320));
                Assert.That(texture.height, Is.EqualTo(200));
                var pixels = texture.GetPixels32();
                Assert.That(pixels[(199 - 32) * 320 + 90].a, Is.EqualTo(255));
                Assert.That(pixels[(199 - 32) * 320 + 90].r, Is.EqualTo(210));
                Assert.That(pixels[(199 - 32) * 320 + 91].a, Is.Zero);
                overlay.Present(frame, palette, tables);
                texture = (Texture2D)field.GetValue(overlay);
                Assert.That(texture.width, Is.EqualTo(64));
                Assert.That(texture.height, Is.EqualTo(64));
            }
            finally { UnityEngine.Object.DestroyImmediate(gameObject); }
        }
    }
}
