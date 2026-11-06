using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernNightLightingEditModeTests
    {
        [Test] public void LightClassificationRequiresTheBoundAssetPairNotAnAnimatedFrame()
        {
            var row = new NativeSim.CampaignEntityPresentation { renderParameterIndex=340,baseTmap=463,selectedTmap=470 };
            Assert.That(ModernNightLighting.SourceKind(row),Is.EqualTo(1));
            row.baseTmap=77;
            Assert.That(ModernNightLighting.SourceKind(row),Is.Zero);
            row.renderParameterIndex=228;
            Assert.That(ModernNightLighting.SourceKind(row),Is.EqualTo(3));
            row.renderParameterIndex=244; row.baseTmap=186;
            Assert.That(ModernNightLighting.SourceKind(row),Is.Zero,"A cold splash must not become a hot emissive fire");
        }

        [Test] public void EveryHouseColorGetsAuthoredTorchButUnknownRowsDoNot()
        {
            for (ushort color=0;color<8;color++)
            {
                var row=new NativeSim.CampaignEntityPresentation {
                    renderParameterIndex=(ushort)(177+color),baseTmap=(ushort)(96+color) };
                Assert.That(ModernNightLighting.SourceKind(row),Is.EqualTo(5));
                row.baseTmap++;
                Assert.That(ModernNightLighting.SourceKind(row),Is.Zero);
            }
            Assert.That(ModernNightLighting.SourceKind(default),Is.Zero);
        }
    }
}
