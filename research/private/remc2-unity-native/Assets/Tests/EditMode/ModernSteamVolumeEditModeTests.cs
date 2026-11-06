using NUnit.Framework;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernSteamVolumeEditModeTests
    {
        private static float[] Read(ModernSteamVolume volume)
        {
            var request = AsyncGPUReadback.Request(volume.State, 0);
            request.WaitForCompletion();
            Assert.That(request.hasError, Is.False);
            // Texture3D readback exposes one z slice per layer, not one flat volume.
            var count = request.layerDataSize / sizeof(float);
            Assert.That(request.layerCount, Is.EqualTo(volume.State.volumeDepth));
            var result = new float[count * request.layerCount];
            for (var z = 0; z < request.layerCount; z++)
            {
                var slice = request.GetData<float>(z);
                for (var i = 0; i < count; i++) result[z * count + i] = slice[i];
            }
            return result;
        }
        private static double Mass(float[] density)
        {
            double sum = 0;
            foreach (var value in density) { Assert.That(float.IsFinite(value) && value >= 0, Is.True); sum += value; }
            return sum * System.Math.Pow(ModernSteamVolume.Size / 32, 3);
        }
        [Test]
        public void QuenchInjectsPositiveDensityAndResetClearsAllChannels()
        {
            using var volume = new ModernSteamVolume(null, 32);
            Assert.That(Mass(Read(volume)), Is.Zero);
            volume.Quench(new Vector3(5, 3, 5), ModernFireballEnergy.Standard);
            for (var i = 0; i < 30; i++) volume.Update(1f / 30, Vector2.zero);
            var mass = Mass(Read(volume));
            var expected = ModernSteamPlume.Evaluate(ModernFireballEnergy.Standard.VaporizedKilograms, 1, 12, .75).CondensedKg;
            // Source mass is analytic, advection is not a conservative fluid solver.
            Assert.That(mass, Is.InRange(expected * .75, expected * 1.25));
            volume.Reset(); Assert.That(Mass(Read(volume)), Is.Zero);
        }
        [Test]
        public void WakeLeavesPersistentDensityChangeAfterSweepAndRejectsTeleport()
        {
            using var control = new ModernSteamVolume(null, 32);
            using var disturbed = new ModernSteamVolume(null, 32);
            var p = new Vector3(5, 3, 5);
            control.Quench(p, ModernFireballEnergy.Standard); disturbed.Quench(p, ModernFireballEnergy.Standard);
            for (var i = 0; i < 45; i++) { control.Update(1f / 30, Vector2.zero); disturbed.Update(1f / 30, Vector2.zero); }
            var center = disturbed.LatestCenter.Value;
            disturbed.Disturb(center - Vector3.right, center + Vector3.right, .2f);
            for (var i = 0; i < 20; i++) { control.Update(1f / 30, Vector2.zero); disturbed.Update(1f / 30, Vector2.zero); }
            var a = Read(control); var b = Read(disturbed); double difference = 0;
            for (var i = 0; i < a.Length; i++) difference += System.Math.Abs(a[i] - b[i]);
            Assert.That(difference, Is.GreaterThan(.01), "Wake must persist after actor leaves");
            Assert.That(Mass(b), Is.GreaterThan(0));
            disturbed.Disturb(center, center + Vector3.right * 300, .01f);
            Assert.That(Mass(Read(disturbed)), Is.EqualTo(Mass(b)), "Invalid sweep must not alter density");
        }
        [Test]
        public void LongTransportDoesNotManufactureCondensate()
        {
            using var volume = new ModernSteamVolume(null, 32);
            volume.Quench(new Vector3(5,3,5), ModernFireballEnergy.Standard);
            for (var frame=1;frame<=840;frame++)
            {
                volume.Update(1f/30,new Vector2(.6f,.2f));
                if(frame%210!=0) continue;
                var mass=Mass(Read(volume));
                var expected=ModernSteamPlume.Evaluate(ModernFireballEnergy.Standard.VaporizedKilograms,frame/30.0,12,.75).CondensedKg;
                TestContext.WriteLine($"age={frame/30f} mass={mass} parcel={expected}");
                Assert.That(mass, Is.LessThanOrEqualTo(expected*1.25+.001), "Transport may lose escaped mass; it must not amplify it without bound");
            }
        }
        [Test]
        public void OverlappingQuenchesShareFieldAndRetainIndependentChannels()
        {
            using var volume = new ModernSteamVolume(null, 32);
            volume.Quench(new Vector3(1023, 3, 4), ModernFireballEnergy.Standard);
            volume.Update(1f / 30, Vector2.zero);
            volume.Quench(new Vector3(-1, 3, 4), ModernFireballEnergy.Standard);
            for (var i = 0; i < 30; i++) volume.Update(1f / 30, Vector2.zero);
            var data = Read(volume); double a=0,b=0,unused=0;
            for(var i=0;i<data.Length;i+=4) { a+=data[i]; b+=data[i+1]; unused+=data[i+2]+data[i+3]; }
            Assert.That(a, Is.GreaterThan(.01)); Assert.That(b, Is.GreaterThan(.01));
            Assert.That(unused, Is.Zero);
            Assert.That(volume.Origin.x, Is.GreaterThan(1000), "World wrap must not drag history across the whole map");
            var origin=volume.Origin;
            volume.PlaceForView(new Vector3(-1,5,4));
            Assert.That(volume.RenderOrigin.x, Is.EqualTo(origin.x-1024).Within(.001));
            Assert.That(volume.Origin, Is.EqualTo(origin), "Only the render copy moves at a world seam");
            Assert.That(Mass(Read(volume)), Is.EqualTo(Mass(data)).Within(.000001));
        }
    }
}
