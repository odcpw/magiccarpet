using System;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class SemanticPresentationAssetEditModeTests
    {
        private const string Slot = "presentation.spell.fireball.projectile_asset";

        private static string PrivateRoot =>
            System.IO.Path.GetFullPath(System.IO.Path.Combine(Application.dataPath, "../.."));

        private static string PackagePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-export-m");

        private static string ClosurePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-asset-closure.json");

        private static string AssetCachePath =>
            System.IO.Path.Combine(PrivateRoot, "generated/level1-unity-asset-cache-plan29-final");

        [Test]
        [Timeout(120000)]
        public void SemanticAssetRebuildsTheActualUnityTextureCacheAndReusesStableIdentity()
        {
            var baselineWorkspace = Environment.GetEnvironmentVariable(
                "MC_SEMANTIC_BASELINE_WORKSPACE");
            var variantWorkspace = Environment.GetEnvironmentVariable(
                "MC_SEMANTIC_VARIANT_WORKSPACE");
            Assert.That(baselineWorkspace, Is.Not.Null.And.Not.Empty);
            Assert.That(variantWorkspace, Is.Not.Null.And.Not.Empty);
            using var baseline = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, baselineWorkspace);
            using var variant = NativeSession.OpenEngine(
                PackagePath, ClosurePath, AssetCachePath, variantWorkspace);
            using var cache = new SemanticPresentationAssetCache();

            var baselineAsset = baseline.EnginePresentationAsset(Slot);
            var baselineTexture = cache.Bind(baselineAsset);
            Assert.That(baselineTexture.width, Is.EqualTo(3));
            Assert.That(baselineTexture.height, Is.EqualTo(3));
            Assert.That(cache.Bind(baseline.EnginePresentationAsset(Slot)),
                Is.SameAs(baselineTexture));
            Assert.That(cache.InvalidationCount, Is.Zero);

            var variantAsset = variant.EnginePresentationAsset(Slot);
            Assert.That(variantAsset.value.content_address,
                Is.Not.EqualTo(baselineAsset.value.content_address));
            var variantTexture = cache.Bind(variantAsset);
            Assert.That(variantTexture, Is.Not.SameAs(baselineTexture));
            Assert.That(cache.InvalidationCount, Is.EqualTo(1));
            Assert.That(variantTexture.filterMode, Is.EqualTo(FilterMode.Point));
            Assert.That(variantTexture.name, Does.Contain(variantAsset.value.content_address));
            Assert.That(cache.Bind(variant.EnginePresentationAsset(Slot)),
                Is.SameAs(variantTexture));
            Assert.That(cache.InvalidationCount, Is.EqualTo(1));
        }
    }
}
