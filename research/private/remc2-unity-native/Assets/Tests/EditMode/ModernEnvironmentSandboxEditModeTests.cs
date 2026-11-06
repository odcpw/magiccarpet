using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernEnvironmentSandboxEditModeTests
    {
        [Test]
        public void WaterExperimentRejectsLandObstructionSkyAndInvalidAim()
        {
            var root=new GameObject("Sandbox targeting test");
            var material=new Material(Shader.Find("Hidden/InternalErrorShader"));
            var heights=new byte[65536]; var types=new byte[65536]; var angles=new byte[65536];
            for(var i=0;i<angles.Length;i++) angles[i]=8;
            using(var terrain=new ModernTerrainMesh(root.transform,material,material))
            {
                terrain.Update(heights,types,angles,1,0);
                var origin=new Vector3(10,10,10); var ray=new Vector3(1,-1,0).normalized;
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin,ray,terrain,out var hit),Is.True);
                Assert.That(hit.y,Is.EqualTo(.15f).Within(.0001));
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin+Vector3.right*1024,ray,terrain,out var wrapped),Is.True);
                Assert.That(wrapped.x-hit.x,Is.EqualTo(1024).Within(.001));
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin,Vector3.up,terrain,out _),Is.False);
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin,new Vector3(float.NaN,-1,0),terrain,out _),Is.False);
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin,new Vector3(1,-.01f,0),terrain,out _),Is.False);
                // A tall dry strip blocks the ray before its otherwise valid lake hit.
                for(var z=0;z<256;z++) { angles[3+z*256]=0; heights[3+z*256]=40; heights[4+z*256]=40; }
                terrain.Update(heights,types,angles,1,1);
                Assert.That(terrain.IsWater(hit.x,hit.z),Is.True);
                Assert.That(ModernEnvironmentSandbox.TryWaterTarget(origin,ray,terrain,out _),Is.False);
            }
            Object.DestroyImmediate(material); Object.DestroyImmediate(root);
        }
    }
}
