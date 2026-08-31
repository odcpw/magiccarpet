using System;
using System.IO;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class CampaignTransitionPresenterEditModeTests
    {
        private const string ExpectedIndicesSha256 =
            "fc27e10c722725e5d4e99ecc01a5bf1f0ff4269e9ff766fdbbad7f424860cc59";
        private const string ExpectedPaletteRgb6Sha256 =
            "7ced7d7bca9414b2890dc086c90efabf424de5694f140262f058ee175f0a1797";
        private const string ExpectedPaletteRgb8Sha256 =
            "4935440c8ca9a71de4ff95131386847014b66f7daee76ddf1f02576d0e7bec27";

        private static string PrivateRoot =>
            Path.GetFullPath(Path.Combine(Application.dataPath, "../.."));

        private static string LevelOnePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_PACKAGE") ??
            Path.Combine(PrivateRoot, "generated/mc-cf15-level1-package-v3-current");

        private static string LevelTwoPackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf02-campaign-package-v2-b/qualified-level2");

        private static string LevelThreePackage =>
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_PACKAGE_LEVEL3_V1") ??
            Path.Combine(
                PrivateRoot,
                "generated/mc-cf14-campaign-package-v1-a/qualified-level3");

        private static string LevelOneAssetCache =>
            Path.Combine(
                PrivateRoot,
                "generated/level1-unity-asset-cache-plan29-final");

        [Test]
        [Timeout(180000)]
        public void QualifiedOwnerFrameFollowsObservedTitleLoadingAndPlayingModes()
        {
            var cache = OwnerAssetCache.OpenAndValidate(
                LevelOneAssetCache,
                MagicCarpetRuntime.ExpectedPackageHash,
                MagicCarpetRuntime.ExpectedOwnerAssetClosureHash);
            var parent = new GameObject("campaign transition presenter test parent");
            try
            {
                using var presenter = new CampaignTransitionPresenter(
                    parent.transform,
                    cache);
                using var runtime = new GameRuntimeTransport();

                Assert.That(presenter.Root.transform.parent, Is.SameAs(parent.transform));
                Assert.That(presenter.IndicesTexture.width,
                    Is.EqualTo(CampaignTransitionPresenter.FrameWidth));
                Assert.That(presenter.IndicesTexture.height,
                    Is.EqualTo(CampaignTransitionPresenter.FrameHeight));
                Assert.That(presenter.IndicesTexture.format, Is.EqualTo(TextureFormat.R8));
                Assert.That(presenter.PaletteTexture.width, Is.EqualTo(256));
                Assert.That(presenter.PaletteTexture.height, Is.EqualTo(1));
                Assert.That(presenter.PaletteTexture.format, Is.EqualTo(TextureFormat.RGB24));
                Assert.That(presenter.IndicesSha256, Is.EqualTo(ExpectedIndicesSha256));
                Assert.That(presenter.PaletteRgb6Sha256,
                    Is.EqualTo(ExpectedPaletteRgb6Sha256));
                Assert.That(presenter.PaletteRgb8Sha256,
                    Is.EqualTo(ExpectedPaletteRgb8Sha256));
                Assert.That(
                    presenter.Material.GetTexture("_Indices"),
                    Is.SameAs(presenter.IndicesTexture));
                Assert.That(
                    presenter.Material.GetTexture("_Palette"),
                    Is.SameAs(presenter.PaletteTexture));

                presenter.Present(runtime.IdentitySnapshot());
                Assert.That(presenter.IsVisible, Is.False, "title is not loading");
                Assert.That(
                    presenter.LastPresentedCueId,
                    Is.EqualTo(GameRuntimeTransport.TitlePresentationCue));

                var loading = runtime.BootstrapPackageSet(
                    Package(LevelThreePackage),
                    Package(LevelOnePackage, LevelOneAssetCache),
                    Package(LevelTwoPackage));
                presenter.Present(loading);
                Assert.That(presenter.IsVisible, Is.True);
                Assert.That(
                    presenter.LastPresentedCueId,
                    Is.EqualTo(
                        "presentation.magic_carpet_2.night_spire.loading"));

                var indicesTexture = presenter.IndicesTexture;
                var paletteTexture = presenter.PaletteTexture;
                var material = presenter.Material;
                for (var poll = 0; poll < 3; poll++)
                {
                    presenter.Present(runtime.IdentitySnapshot());
                    Assert.That(presenter.IsVisible, Is.True);
                    Assert.That(presenter.IndicesTexture, Is.SameAs(indicesTexture));
                    Assert.That(presenter.PaletteTexture, Is.SameAs(paletteTexture));
                    Assert.That(presenter.Material, Is.SameAs(material));
                }

                presenter.Present(runtime.OpenLevel());
                Assert.That(presenter.IsVisible, Is.False, "playing hides loading");
                presenter.Present(runtime.Pause());
                Assert.That(presenter.IsVisible, Is.False, "paused hides loading");

                var destination = new GameRuntimeTransport.RuntimeFlowDestination
                {
                    kind = "level",
                    level_id = "level.magic_carpet_2.payahandra_ruins"
                };
                var completionCue = new GameRuntimeTransport.RuntimePresentationCue
                {
                    cue_id =
                        "presentation.magic_carpet_2.night_spire.completion",
                    phase = GameRuntimeTransport.LevelCompletionPresentationPhase,
                    level_id = "level.magic_carpet_2.night_spire",
                    completion_destination = destination
                };
                var nextLoadingCue = new GameRuntimeTransport.RuntimePresentationCue
                {
                    cue_id =
                        "presentation.magic_carpet_2.payahandra_ruins.loading",
                    phase = GameRuntimeTransport.LoadingPresentationPhase,
                    level_id = "level.magic_carpet_2.payahandra_ruins"
                };
                presenter.Present(new GameRuntimeTransport.CompletionTransitionReceipt
                {
                    completed_level_id = "level.magic_carpet_2.night_spire",
                    completion_destination = destination,
                    after = new GameRuntimeTransport.Identity
                    {
                        presentation = nextLoadingCue
                    },
                    presentation_sequence = new[]
                    {
                        completionCue,
                        nextLoadingCue
                    }
                });
                Assert.That(
                    presenter.LastPresentationSequence,
                    Is.EqualTo(new[]
                    {
                        "presentation.magic_carpet_2.night_spire.completion",
                        "presentation.magic_carpet_2.payahandra_ruins.loading"
                    }));
                Assert.That(presenter.LastPresentedCueId,
                    Is.EqualTo(nextLoadingCue.cue_id));
                Assert.That(presenter.IsVisible, Is.True,
                    "the ordered Rust receipt ends on its selected Loading cue");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        [Test]
        public void UnknownAndMalformedRustCuesFailClosed()
        {
            var parent = new GameObject("unknown transition cue test parent");
            try
            {
                using var presenter = new CampaignTransitionPresenter(
                    parent.transform,
                    new byte[CampaignTransitionPresenter.FrameWidth *
                             CampaignTransitionPresenter.FrameHeight],
                    new byte[256 * 3]);
                Assert.Throws<InvalidDataException>(() => presenter.Present(
                    new GameRuntimeTransport.Identity
                    {
                        mode = GameRuntimeTransport.LoadingMode,
                        presentation = new GameRuntimeTransport.RuntimePresentationCue
                        {
                            cue_id =
                                "presentation.magic_carpet_2.unknown.loading",
                            phase = GameRuntimeTransport.LoadingPresentationPhase,
                            level_id = "level.magic_carpet_2.unknown"
                        },
                        pending_definition = new GameRuntimeTransport.DefinitionIdentity
                        {
                            runtime_flow = new GameRuntimeTransport.RuntimeFlowDefinition
                            {
                                bound_level_id = "level.magic_carpet_2.unknown"
                            }
                        }
                    }));
                Assert.That(presenter.IsVisible, Is.False);

                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidatePresentationCue(
                        new GameRuntimeTransport.RuntimePresentationCue
                        {
                            cue_id = "presentation.magic_carpet_2.night_spire.loading",
                            phase = "invented",
                            level_id = "level.magic_carpet_2.night_spire"
                        }));
                Assert.That(presenter.IsVisible, Is.False);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        [Test]
        public void MalformedOwnerFrameFailsBeforeCreatingPresentationObjects()
        {
            var parent = new GameObject("malformed transition frame test parent");
            try
            {
                Assert.Throws<InvalidDataException>(() =>
                    new CampaignTransitionPresenter(
                        parent.transform,
                        new byte[CampaignTransitionPresenter.FrameWidth *
                                 CampaignTransitionPresenter.FrameHeight - 1],
                        new byte[256 * 3]));
                Assert.That(parent.transform.childCount, Is.Zero);

                var invalidRgb6 = new byte[256 * 3];
                invalidRgb6[17] = 64;
                Assert.Throws<InvalidDataException>(() =>
                    new CampaignTransitionPresenter(
                        parent.transform,
                        new byte[CampaignTransitionPresenter.FrameWidth *
                                 CampaignTransitionPresenter.FrameHeight],
                        invalidRgb6));
                Assert.That(parent.transform.childCount, Is.Zero);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        private static GameRuntimeTransport.InstalledPackageResource Package(
            string packagePath,
            string ownerCachePath = null)
        {
            return new GameRuntimeTransport.InstalledPackageResource
            {
                package_dir = packagePath,
                owner_cache_path = ownerCachePath
            };
        }
    }
}
