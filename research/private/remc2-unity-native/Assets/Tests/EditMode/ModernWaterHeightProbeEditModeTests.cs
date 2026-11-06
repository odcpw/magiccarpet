using System.Collections;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernWaterHeightProbeEditModeTests
    {
        [Test]
        public void StaleOutsideAndChangedCoastSamplesAreRejected()
        {
            var center = new Vector2(100,200);
            Assert.That(ModernWaterHeightProbe.IsUsable(center+Vector2.right*10,center,.1f,7,7), Is.True);
            Assert.That(ModernWaterHeightProbe.IsUsable(center+Vector2.right*10.01f,center,.1f,7,7), Is.False);
            Assert.That(ModernWaterHeightProbe.IsUsable(center,center,.151f,7,7), Is.False);
            Assert.That(ModernWaterHeightProbe.IsUsable(center,center,-.01f,7,7), Is.False);
            Assert.That(ModernWaterHeightProbe.IsUsable(center,center,.1f,8,7), Is.False);
            Assert.That(ModernWaterHeightProbe.IsUsable(new Vector2(float.NaN,0),center,.1f,7,7), Is.False);
        }

        [UnityTest]
        public IEnumerator ActualGpuReductionAndReadbackFollowRenderedSurfaceAndRejectStaleResults()
        {
            Assert.That(SystemInfo.supportsComputeShaders && SystemInfo.supportsAsyncGPUReadback, Is.True,
                "The integration test requires real asynchronous compute readback");
            var broad = BroadWithPeak();
            var coast = Solid(new Color(20,0,0,0),TextureWrapMode.Repeat);
            var local = Solid(new Color(.4f,0,0,0),TextureWrapMode.Clamp);
            using var probe = new ModernWaterHeightProbe();
            try
            {
                Assert.That(probe.TryClearance(Vector2.zero,10,3,out _), Is.False);
                probe.Submit(Vector2.zero,10,3,broad,coast,local,new Vector4(-64,-64,128,1f/128),Vector4.zero,0);
                var ready = false; var clearance = 0f;
                for (var frame=0;frame<300 && !ready;frame++)
                {
                    yield return null;
                    ready = probe.TryClearance(Vector2.zero,10,3,out clearance);
                }
                Assert.That(ready, Is.True, "The real GPU request must complete");
                // Peak is 8m away: .02 datum + 2.2 broad + .4 local;
                // zero wind means no shore wave. A center-only probe would miss it.
                Assert.That(clearance, Is.EqualTo(3.62f).Within(.002f));
                Assert.That(probe.TryClearance(Vector2.zero,10.151f,3,out _), Is.False);
                Assert.That(probe.TryClearance(new Vector2(11,0),10,3,out _), Is.False);
                Assert.That(probe.TryClearance(Vector2.zero,10,4,out _), Is.False);

                probe.Submit(Vector2.zero,10,3,broad,coast,local,new Vector4(-64,-64,128,1f/128),Vector4.zero,0);
                Assert.That(probe.Pending, Is.True);
                probe.Invalidate();
                Assert.That(probe.TryClearance(Vector2.zero,10,3,out _), Is.False, "Reset rejects the cached sample immediately");
                for (var frame=0;frame<300 && probe.Pending;frame++) yield return null;
                Assert.That(probe.Pending, Is.False, "The invalidated request must finish");
                Assert.That(probe.TryClearance(Vector2.zero,10,3,out _), Is.False, "An older callback must not republish after reset");

                probe.Submit(Vector2.zero,10,3,broad,coast,local,new Vector4(-64,-64,128,1f/128),Vector4.zero,0);
                Assert.That(probe.Pending, Is.True);
                probe.Dispose();
                Assert.That(probe.Pending, Is.False, "Disposal must drain the outstanding request before returning");
                Assert.That(probe.BufferReleased, Is.True, "Disposal must release without relying on a future frame callback");
                yield return null;
                probe.Dispose();
                Assert.That(probe.BufferReleased, Is.True, "A late callback or repeated disposal must be harmless");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(broad); UnityEngine.Object.DestroyImmediate(coast);
                UnityEngine.Object.DestroyImmediate(local);
            }
        }

        private static Texture2D Solid(Color color, TextureWrapMode wrap)
        {
            var texture = new Texture2D(2,2,TextureFormat.RGBAFloat,false,true) { filterMode=FilterMode.Bilinear,wrapMode=wrap };
            texture.SetPixels(new[] {color,color,color,color}); texture.Apply(); return texture;
        }

        private static Texture2D BroadWithPeak()
        {
            var texture = new Texture2D(256,256,TextureFormat.RGBAFloat,false,true) {
                filterMode=FilterMode.Bilinear,wrapMode=TextureWrapMode.Repeat };
            var pixels = new Color[256*256];
            for (var i=0;i<pixels.Length;i++) pixels[i]=new Color(0,.7f,0,0);
            pixels[8+4*256]=new Color(0,2.2f,0,0);
            texture.SetPixels(pixels); texture.Apply(); return texture;
        }
    }
}
