using System;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernFireballEnergyEditModeTests
    {
        [Test] public void StandardEnergyConservesTransferredHeatAndSeparatesVaporFromCloud()
        {
            var e = ModernFireballEnergy.Standard;
            Assert.That(e.VaporizedKilograms, Is.InRange(.771, .772));
            Assert.That(e.VaporizedKilograms * (2675600 - 4192 * 20), Is.EqualTo(2000000).Within(.0001));
            Assert.That(e.SteamCubicMetres, Is.InRange(1.29, 1.30));
        }

        [Test] public void EightTimesEnergyMakesEightTimesTheVaporMass()
        {
            var small = ModernFireballEnergy.Standard;
            var large = new ModernFireballEnergy(small.Joules * 8);
            Assert.That(large.VaporizedKilograms, Is.EqualTo(small.VaporizedKilograms * 8).Within(1e-9));
        }

        [Test] public void NoTransferredHeatCannotVaporizeWaterAndInvalidInputsReject()
        {
            Assert.That(new ModernFireballEnergy(2e6, waterCoupling: 0).VaporizedKilograms, Is.Zero);
            Assert.Throws<ArgumentOutOfRangeException>(() => new ModernFireballEnergy(double.NaN));
            Assert.Throws<ArgumentOutOfRangeException>(() => new ModernFireballEnergy(2e6, waterCoupling: 1.01));
        }
    }
}
