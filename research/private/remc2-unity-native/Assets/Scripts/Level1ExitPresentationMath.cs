using System;
using UnityEngine;

namespace MagicCarpet.Native
{
    internal static class Level1ExitPresentationMath
    {
        private const int CorrectedDisplayHeight = 240;
        private const int SourceDiagonalRoot = 377;

        internal static float UnityVerticalFov(short sourceHistoryFov)
        {
            if (sourceHistoryFov <= 0)
                throw new ArgumentOutOfRangeException(
                    nameof(sourceHistoryFov),
                    sourceHistoryFov,
                    "source camera FOV must be positive");
            var sourceScale =
                7 * SourceDiagonalRoot * sourceHistoryFov >> 11;
            if (sourceScale <= 0)
                throw new ArgumentOutOfRangeException(
                    nameof(sourceHistoryFov),
                    sourceHistoryFov,
                    "source camera projection scale must be positive");
            return 2f * Mathf.Atan(
                (CorrectedDisplayHeight * 0.5f) / sourceScale) *
                Mathf.Rad2Deg;
        }

        internal static Rect SourceDisplayRect(int width, int height)
        {
            if (width <= 0) throw new ArgumentOutOfRangeException(nameof(width));
            if (height <= 0) throw new ArgumentOutOfRangeException(nameof(height));
            var screenAspect = width / (float)height;
            var sourceAspect = 4f / 3f;
            if (screenAspect > sourceAspect)
            {
                var normalizedWidth = sourceAspect / screenAspect;
                return new Rect(
                    (1f - normalizedWidth) * 0.5f,
                    0f,
                    normalizedWidth,
                    1f);
            }
            var normalizedHeight = screenAspect / sourceAspect;
            return new Rect(
                0f,
                (1f - normalizedHeight) * 0.5f,
                1f,
                normalizedHeight);
        }

        internal static void MaterializePalette(
            byte[] originalRgb8,
            byte effect,
            byte weight,
            byte denominator,
            byte[] destinationRgb8)
        {
            if (
                originalRgb8 == null ||
                destinationRgb8 == null ||
                originalRgb8.Length != 256 * 3 ||
                destinationRgb8.Length != originalRgb8.Length)
                throw new ArgumentException("PALN palette must contain exactly 256 RGB rows");
            if (weight > denominator)
                throw new ArgumentOutOfRangeException(nameof(weight));

            for (var offset = 0; offset < originalRgb8.Length; offset += 3)
            {
                var red = originalRgb8[offset] >> 2;
                var green = originalRgb8[offset + 1] >> 2;
                var blue = originalRgb8[offset + 2] >> 2;
                var outputRed = red;
                var outputGreen = green;
                var outputBlue = blue;
                if (effect != NativeSim.Level1ExitPaletteNone && offset == 0)
                {
                    // PaletteChanges writes transient palettes into a
                    // zero-initialized work buffer and starts at row one.
                    outputRed = 0;
                    outputGreen = 0;
                    outputBlue = 0;
                }
                else if (
                    effect == NativeSim.Level1ExitPaletteEntryFlash &&
                    denominator == 4)
                {
                    var flashRed = Math.Min(63, blue + 48);
                    var flashGreen = Math.Min(63, green + 32);
                    var flashBlue = Math.Min(63, blue + 32);
                    outputRed = red + (flashRed - red) * weight / denominator;
                    outputGreen =
                        green + (flashGreen - green) * weight / denominator;
                    outputBlue = blue + (flashBlue - blue) * weight / denominator;
                }
                else if (
                    effect == NativeSim.Level1ExitPaletteBlack &&
                    denominator == 28)
                {
                    var remaining = denominator - weight;
                    outputRed = red * remaining / denominator;
                    outputGreen = green * remaining / denominator;
                    outputBlue = blue * remaining / denominator;
                }
                else if (
                    effect != NativeSim.Level1ExitPaletteNone ||
                    weight != 0 ||
                    denominator != 0)
                {
                    throw new ArgumentException(
                        $"unsupported palette state {effect}/{weight}/{denominator}");
                }
                destinationRgb8[offset] = checked((byte)(outputRed << 2));
                destinationRgb8[offset + 1] = checked((byte)(outputGreen << 2));
                destinationRgb8[offset + 2] = checked((byte)(outputBlue << 2));
            }
        }

        internal static Color32 MaterializeColour(
            Color32 original,
            byte effect,
            byte weight,
            byte denominator)
        {
            var source = new[] { original.r, original.g, original.b };
            var destination = new byte[3];
            MaterializeColour(source, effect, weight, denominator, destination);
            return new Color32(
                destination[0],
                destination[1],
                destination[2],
                original.a);
        }

        private static void MaterializeColour(
            byte[] original,
            byte effect,
            byte weight,
            byte denominator,
            byte[] destination)
        {
            var red = original[0] >> 2;
            var green = original[1] >> 2;
            var blue = original[2] >> 2;
            var outputRed = red;
            var outputGreen = green;
            var outputBlue = blue;
            if (
                effect == NativeSim.Level1ExitPaletteEntryFlash &&
                denominator == 4)
            {
                outputRed = red + (Math.Min(63, blue + 48) - red) *
                    weight / denominator;
                outputGreen = green + (Math.Min(63, green + 32) - green) *
                    weight / denominator;
                outputBlue = blue + (Math.Min(63, blue + 32) - blue) *
                    weight / denominator;
            }
            else if (
                effect == NativeSim.Level1ExitPaletteBlack &&
                denominator == 28)
            {
                var remaining = denominator - weight;
                outputRed = red * remaining / denominator;
                outputGreen = green * remaining / denominator;
                outputBlue = blue * remaining / denominator;
            }
            else if (
                effect != NativeSim.Level1ExitPaletteNone ||
                weight != 0 ||
                denominator != 0)
            {
                throw new ArgumentException(
                    $"unsupported palette state {effect}/{weight}/{denominator}");
            }
            destination[0] = checked((byte)(outputRed << 2));
            destination[1] = checked((byte)(outputGreen << 2));
            destination[2] = checked((byte)(outputBlue << 2));
        }
    }
}
