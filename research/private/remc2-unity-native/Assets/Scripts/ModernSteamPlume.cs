using System;

namespace MagicCarpet.Native
{
    // A bounded, spatially uniform moist parcel for cosmetic quench plumes. This
    // is constant-pressure equilibrium mixing, not resolved CFD or cloud microphysics.
    // Entrained air brings its own water and enthalpy; condensation releases latent
    // heat into the same solve. Expansion follows gas volume rather than a scale curve.
    internal static class ModernSteamPlume
    {
        internal const double PressurePa = 101325;
        internal const double DryAirGasConstant = 287.05;
        internal const double VaporGasConstant = 461.5;
        internal const double DryAirHeatCapacity = 1005;
        internal const double VaporHeatCapacity = 1860;
        internal const double LiquidHeatCapacity = 4192;
        internal const double SteamEnthalpy100 = 2675600;
        private const double VaporEnthalpyZero = SteamEnthalpy100 - VaporHeatCapacity * 100;
        private const double LiquidDensity = 1000;
        private const int SolveIterations = 48;

        internal readonly struct State
        {
            internal readonly double TemperatureC, VaporKg, CondensedKg, DryAirKg;
            internal readonly double AmbientWaterKg, VolumeM3, RadiusM, EnthalpyJ, InputEnthalpyJ;
            internal double TotalWaterKg => VaporKg + CondensedKg;
            internal double LiquidWaterContentKgM3 => VolumeM3 > 0 ? CondensedKg / VolumeM3 : 0;
            internal double DensityKgM3 => VolumeM3 > 0 ? (DryAirKg + TotalWaterKg) / VolumeM3 : 0;

            internal State(double temperatureC, double vaporKg, double condensedKg, double dryAirKg,
                double ambientWaterKg, double inputEnthalpyJ)
            {
                TemperatureC = temperatureC; VaporKg = vaporKg; CondensedKg = condensedKg;
                DryAirKg = dryAirKg; AmbientWaterKg = ambientWaterKg; InputEnthalpyJ = inputEnthalpyJ;
                // Ideal gas mixture + negligible but retained liquid droplet volume.
                VolumeM3 = (dryAirKg * DryAirGasConstant + vaporKg * VaporGasConstant)
                    * (temperatureC + 273.15) / PressurePa + condensedKg / LiquidDensity;
                RadiusM = Math.Pow(VolumeM3 * 3 / (4 * Math.PI), 1.0 / 3.0);
                EnthalpyJ = Enthalpy(temperatureC, dryAirKg, vaporKg, condensedKg);
            }
        }

        // The entrainment coefficient is an authored mixing assumption: kg of dry
        // ambient air per kg of initial steam per second. It is not inferred from
        // wind or declared a universal plume law. No state, allocation or per-puff
        // solve is required: evaluate once per active plume, then share the result.
        internal static State Evaluate(double initialSteamKg, double ageSeconds,
            double ambientTemperatureC = 20, double relativeHumidity = .6,
            double dryAirEntrainmentPerSteamSecond = 8)
        {
            RequireRange(initialSteamKg, 0, 1000, nameof(initialSteamKg));
            RequireRange(ageSeconds, 0, 3600, nameof(ageSeconds));
            RequireRange(ambientTemperatureC, 0, 45, nameof(ambientTemperatureC));
            RequireRange(relativeHumidity, 0, 1, nameof(relativeHumidity));
            RequireRange(dryAirEntrainmentPerSteamSecond, 0, 100, nameof(dryAirEntrainmentPerSteamSecond));
            if (initialSteamKg == 0) return new State(ambientTemperatureC, 0, 0, 0, 0, 0);

            var dryAir = initialSteamKg * dryAirEntrainmentPerSteamSecond * ageSeconds;
            var ambientWater = dryAir * AmbientWaterPerDryAir(ambientTemperatureC, relativeHumidity);
            var totalWater = initialSteamKg + ambientWater;
            var incoming = initialSteamKg * SteamEnthalpy100
                + Enthalpy(ambientTemperatureC, dryAir, ambientWater, 0);
            if (dryAir == 0) return new State(100, initialSteamKg, 0, 0, 0, incoming);

            // H(T) is monotone: positive sensible heat capacities plus latent heat
            // released/absorbed by the saturation partition. The mixture is bracketed
            // by ambient air and the initial 100C steam. Fixed work, independent of age.
            var lower = ambientTemperatureC; var upper = 100.0;
            for (var iteration = 0; iteration < SolveIterations; iteration++)
            {
                var temperature = (lower + upper) * .5;
                var vapor = EquilibriumVaporKg(temperature, dryAir, totalWater);
                if (Enthalpy(temperature, dryAir, vapor, totalWater - vapor) < incoming) lower = temperature;
                else upper = temperature;
            }
            var finalTemperature = (lower + upper) * .5;
            var finalVapor = EquilibriumVaporKg(finalTemperature, dryAir, totalWater);
            return new State(finalTemperature, finalVapor, Math.Max(0, totalWater - finalVapor),
                dryAir, ambientWater, incoming);
        }

        // At the constant-pressure interface, available heat first warms source water
        // then vaporizes it. Coupling/energy are artistic inputs, not MC2 damage units.
        internal static double SteamMassFromEnergy(double energyJoules, double waterCoupling, double waterTemperatureC = 20)
        {
            RequireRange(energyJoules, 0, 1e9, nameof(energyJoules));
            RequireRange(waterCoupling, 0, 1, nameof(waterCoupling));
            RequireRange(waterTemperatureC, 0, 100, nameof(waterTemperatureC));
            return energyJoules * waterCoupling / (SteamEnthalpy100 - LiquidHeatCapacity * waterTemperatureC);
        }

        internal static double AmbientWaterPerDryAir(double temperatureC, double relativeHumidity)
        {
            var pressure = relativeHumidity * SaturationPressurePa(temperatureC);
            return (DryAirGasConstant / VaporGasConstant) * pressure / (PressurePa - pressure);
        }

        internal static double SaturationPressurePa(double temperatureC)
        {
            // Buck (1996) over liquid water. Its compact approximation is normalized
            // by 1.00017 so 100C is exactly this model's 1-atm boiling anchor. This
            // avoids a tiny inconsistent near-pure-vapor branch; it is not IF97.
            // Formula: https://www.eol.ucar.edu/data-software/conventions-and-standards/water-vapor-pressure-formulations
            return 611.21 * 1.0001699682158955
                * Math.Exp((18.678 - temperatureC / 234.5) * temperatureC / (257.14 + temperatureC));
        }

        private static double EquilibriumVaporKg(double temperatureC, double dryAir, double water)
        {
            var saturation = SaturationPressurePa(temperatureC);
            if (saturation >= PressurePa) return water;
            var capacity = dryAir * (DryAirGasConstant / VaporGasConstant) * saturation / (PressurePa - saturation);
            return Math.Min(water, capacity);
        }

        private static double Enthalpy(double temperatureC, double dryAir, double vapor, double liquid) =>
            dryAir * DryAirHeatCapacity * temperatureC + vapor * (VaporEnthalpyZero + VaporHeatCapacity * temperatureC)
            + liquid * LiquidHeatCapacity * temperatureC;

        private static void RequireRange(double value, double minimum, double maximum, string name)
        {
            if (double.IsNaN(value) || double.IsInfinity(value) || value < minimum || value > maximum)
                throw new ArgumentOutOfRangeException(name, $"Expected finite value from {minimum} through {maximum}");
        }
    }
}
