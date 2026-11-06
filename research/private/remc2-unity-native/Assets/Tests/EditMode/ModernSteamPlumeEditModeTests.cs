using System;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernSteamPlumeEditModeTests
    {
        [Test]
        public void HeatBudgetIncludesWarmingAndLatentHeat()
        {
            var upper = ModernSteamPlume.SteamMassFromEnergy(2e6, 1, 20);
            var coupled = ModernSteamPlume.SteamMassFromEnergy(2e6, .35, 20);
            Assert.That(upper, Is.EqualTo(.7716763913).Within(.000001));
            Assert.That(coupled, Is.EqualTo(upper * .35).Within(1e-12));
            Assert.That(ModernSteamPlume.SteamMassFromEnergy(2e6, 0, 20), Is.Zero);
            Assert.That(ModernSteamPlume.SteamMassFromEnergy(2e6, 1, 100), Is.GreaterThan(upper));
        }

        [Test]
        public void PureSteamStartsHotWithIdealGasVolumeAndNoInventedCondensate()
        {
            var state = ModernSteamPlume.Evaluate(.77, 0);
            Assert.That(state.TemperatureC, Is.EqualTo(100));
            Assert.That(state.VaporKg, Is.EqualTo(.77));
            Assert.That(state.CondensedKg, Is.Zero);
            Assert.That(state.DryAirKg, Is.Zero);
            Assert.That(state.VolumeM3, Is.EqualTo(.77 * 461.5 * 373.15 / 101325).Within(1e-12));
            Assert.That(state.EnthalpyJ, Is.EqualTo(.77 * 2675600).Within(1e-8));
            var empty = ModernSteamPlume.Evaluate(0, 3);
            Assert.That(empty.VolumeM3, Is.Zero);
            Assert.That(empty.LiquidWaterContentKgM3, Is.Zero);
        }

        [Test]
        public void HumidAirMixingPreservesWaterAndEnthalpyAcrossCondensation()
        {
            foreach (var temperature in new[] { 0.0, 20.0, 45.0 })
            foreach (var humidity in new[] { 0.0, .6, 1.0 })
            foreach (var age in new[] { .001, .1, 1.0, 5.0, 30.0, 60.0 })
            {
                var state = ModernSteamPlume.Evaluate(.7717, age, temperature, humidity);
                var incomingWater = .7717 + state.DryAirKg * ModernSteamPlume.AmbientWaterPerDryAir(temperature, humidity);
                Assert.That(state.TotalWaterKg, Is.EqualTo(incomingWater).Within(1e-10));
                Assert.That(state.EnthalpyJ, Is.EqualTo(state.InputEnthalpyJ).Within(Math.Max(1e-5, state.InputEnthalpyJ * 1e-10)));
                Assert.That(state.TemperatureC, Is.InRange(temperature, 100));
                Assert.That(state.CondensedKg, Is.InRange(0, incomingWater));
                var partialPressure = ModernSteamPlume.PressurePa * (state.VaporKg * ModernSteamPlume.VaporGasConstant)
                    / (state.DryAirKg * ModernSteamPlume.DryAirGasConstant + state.VaporKg * ModernSteamPlume.VaporGasConstant);
                var saturation = ModernSteamPlume.SaturationPressurePa(state.TemperatureC);
                if (state.CondensedKg > 1e-9) Assert.That(partialPressure, Is.EqualTo(saturation).Within(1e-7));
                else Assert.That(partialPressure, Is.LessThanOrEqualTo(saturation + 1e-7));
            }
        }

        [Test]
        public void PlumeExpandsCoolsAndLaterReevaporatesInUnsaturatedAir()
        {
            var initial = ModernSteamPlume.Evaluate(.77, 0);
            var young = ModernSteamPlume.Evaluate(.77, 1);
            var mature = ModernSteamPlume.Evaluate(.77, 4);
            var old = ModernSteamPlume.Evaluate(.77, 40);
            Assert.That(young.RadiusM, Is.GreaterThan(initial.RadiusM));
            Assert.That(mature.RadiusM, Is.GreaterThan(young.RadiusM));
            Assert.That(old.RadiusM, Is.GreaterThan(mature.RadiusM));
            Assert.That(young.TemperatureC, Is.LessThan(initial.TemperatureC));
            Assert.That(mature.TemperatureC, Is.LessThan(young.TemperatureC));
            Assert.That(old.TemperatureC, Is.LessThan(mature.TemperatureC));
            Assert.That(young.CondensedKg, Is.GreaterThan(0));
            Assert.That(mature.CondensedKg, Is.GreaterThan(young.CondensedKg));
            Assert.That(old.CondensedKg, Is.Zero);
            var humid = ModernSteamPlume.Evaluate(.77, 4, 20, 1);
            var dry = ModernSteamPlume.Evaluate(.77, 4, 20, 0);
            Assert.That(humid.CondensedKg, Is.GreaterThan(dry.CondensedKg));
        }

        [Test]
        public void ExtensiveScalingPreservesTemperatureAndCubeRootRadius()
        {
            var small = ModernSteamPlume.Evaluate(.1, 3);
            var big = ModernSteamPlume.Evaluate(.8, 3);
            Assert.That(big.TemperatureC, Is.EqualTo(small.TemperatureC).Within(1e-10));
            Assert.That(big.CondensedKg, Is.EqualTo(small.CondensedKg * 8).Within(1e-10));
            Assert.That(big.VolumeM3, Is.EqualTo(small.VolumeM3 * 8).Within(1e-10));
            Assert.That(big.RadiusM, Is.EqualTo(small.RadiusM * 2).Within(1e-10));
        }

        [Test]
        public void InvalidPhysicalInputsRefuseInsteadOfProducingNan()
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.Evaluate(double.NaN, 1));
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.Evaluate(.1, -1));
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.Evaluate(.1, 1, -10));
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.Evaluate(.1, 1, 20, 1.1));
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.Evaluate(.1, 1, 20, .6, double.PositiveInfinity));
            Assert.Throws<ArgumentOutOfRangeException>(() => ModernSteamPlume.SteamMassFromEnergy(-1, 1));
        }
    }
}
