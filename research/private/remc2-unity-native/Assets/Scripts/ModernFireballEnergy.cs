using System;

namespace MagicCarpet.Native
{
    // Presentation calibration, not recovered MC2 physics. One rendered unit is treated
    // as one metre. Magic stores energy: radius/temperature alone cannot determine it.
    internal readonly struct ModernFireballEnergy
    {
        internal const double ReferenceDiameterMetres = .65;
        internal const double ReferenceJoules = 2_000_000;
        internal readonly double Joules, DiameterMetres, VaporizedKilograms;
        internal readonly double SteamCubicMetres;

        internal ModernFireballEnergy(double joules, double diameterMetres = ReferenceDiameterMetres,
            double waterCoupling = 1, double waterCelsius = 20)
        {
            if (!Finite(joules) || joules < 0 || !Finite(diameterMetres) || diameterMetres <= 0
                || !Fraction(waterCoupling)
                || !Finite(waterCelsius) || waterCelsius < 0 || waterCelsius > 100)
                throw new ArgumentOutOfRangeException(nameof(joules), "Invalid fireball/water calibration");
            Joules = joules; DiameterMetres = diameterMetres;
            // Atmospheric-pressure enthalpy model shared with the plume; saturated
            // steam volume approximately1.673 m3/kg at100C.
            // NIST/IAPWS tables: https://www.nist.gov/srd/nistir-5078
            // Fully quenched default transfers all remaining thermal energy to water.
            // Vapor yield is an upper bound: warming a larger surrounding volume without
            // boiling, radiation and mechanical work reduce real evaporated mass.
            VaporizedKilograms = ModernSteamPlume.SteamMassFromEnergy(joules, waterCoupling, waterCelsius);
            SteamCubicMetres = VaporizedKilograms * 1.673;
        }

        internal static ModernFireballEnergy Standard => new ModernFireballEnergy(ReferenceJoules);
        private static bool Finite(double value) => !double.IsNaN(value) && !double.IsInfinity(value);
        private static bool Fraction(double value) => Finite(value) && value >= 0 && value <= 1;
    }
}
