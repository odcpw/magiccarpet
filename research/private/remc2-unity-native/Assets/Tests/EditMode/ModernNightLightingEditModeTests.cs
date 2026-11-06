using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernNightLightingEditModeTests
    {
        [Test] public void WindowLightsShareBudgetWrapAndSwitchOffInDaylight()
        {
            var root=new GameObject("Window lighting test");var camera=new GameObject("Window camera").AddComponent<Camera>();
            camera.transform.position=new Vector3(1024,2,0);
            using(var lighting=new ModernNightLighting(root.transform))
            {
                var centers=new Vector3[12];var normals=new Vector3[12];
                for(var i=0;i<12;i++) { centers[i]=new Vector3(i,2,0);normals[i]=Vector3.back; }
                lighting.SetReviewWindows(centers,normals);lighting.SetWindowLighting(.8f);lighting.Render(camera);
                Assert.That(Shader.GetGlobalInt("_ModernLightCount"),Is.EqualTo(8));
                var positions=Shader.GetGlobalVectorArray("_ModernLightPosition");
                var directions=Shader.GetGlobalVectorArray("_ModernLightDirection");
                for(var i=0;i<8;i++)
                {
                    Assert.That(positions[i].x,Is.InRange(1024,1035));
                    Assert.That(directions[i].z,Is.EqualTo(-1));Assert.That(directions[i].w,Is.EqualTo(1));
                }
                Assert.That(root.GetComponentsInChildren<MeshRenderer>().Length,Is.Zero,"Windows must not create torch fixtures");
                lighting.SetWindowLighting(0);lighting.Render(camera);
                Assert.That(Shader.GetGlobalInt("_ModernLightCount"),Is.Zero);
            }
            Object.DestroyImmediate(root);Object.DestroyImmediate(camera.gameObject);
        }
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
