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
            Environment.GetEnvironmentVariable("MC_CAMPAIGN_LEVEL1_CACHE") ??
            Path.Combine(
                PrivateRoot,
                "generated/level1-unity-asset-cache-plan29-final");

        [Serializable]
        private sealed class NightMetadataIdentity
        {
            public string night_metadata_sha256;
        }

        [Test]
        public void CurrentNightCacheRejectsAlteredMetadataBeforeReadingAssets()
        {
            var source = Path.Combine(PrivateRoot, "generated/mc2-level1-night-cache-v2/manifest.json");
            var original = File.ReadAllText(source);
            var digest = JsonUtility.FromJson<NightMetadataIdentity>(original).night_metadata_sha256;
            Assert.That(digest?.Length, Is.EqualTo(64));
            var directory = Path.Combine(Path.GetTempPath(), "mc-night-metadata-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(directory);
            try
            {
                File.WriteAllText(Path.Combine(directory, "manifest.json"), original.Replace(digest, new string('f', 64)));
                var error = Assert.Throws<InvalidDataException>(() => OwnerAssetCache.OpenAndValidate(
                    directory, MagicCarpetRuntime.ExpectedPackageHash, MagicCarpetRuntime.ExpectedOwnerAssetClosureHash));
                Assert.That(error.Message, Does.Contain("Night cache metadata identity mismatch"),
                    "metadata must be rejected before the deliberately absent output tree is read");
            }
            finally { Directory.Delete(directory, true); }
        }

        [Test]
        [Timeout(180000)]
        public void QualifiedOwnerFrameFollowsObservedTitleLoadingAndPlayingModes()
        {
            var cache = OwnerAssetCache.OpenAndValidate(
                LevelOneAssetCache,
                MagicCarpetRuntime.ExpectedPackageHash,
                MagicCarpetRuntime.ExpectedOwnerAssetClosureHash,
                LevelOnePackage);
            Assert.Throws<InvalidDataException>(() => OwnerAssetCache.OpenAndValidate(
                LevelOneAssetCache, MagicCarpetRuntime.ExpectedPackageHash,
                MagicCarpetRuntime.ExpectedOwnerAssetClosureHash, LevelTwoPackage),
                "restored Night presentation must reject a different selected package");
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
                var selectedResource = Package(LevelOnePackage, LevelOneAssetCache);
                selectedResource.presentation_cache_path = LevelOneAssetCache;
                Assert.That(CampaignTransitionPresenter.PreflightDestination(
                    presenter.Destination, new[] { selectedResource }), Is.SameAs(selectedResource));
                var exactCapture = presenter.Destination.runtime_capture_sha256;
                presenter.Destination.runtime_capture_sha256 = new string('f', 64);
                Assert.Throws<InvalidDataException>(() => CampaignTransitionPresenter.PreflightDestination(
                    presenter.Destination, new[] { selectedResource }));
                presenter.Destination.runtime_capture_sha256 = exactCapture;
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

                presenter.Present(runtime.Close());
                Assert.That(presenter.IsVisible, Is.False, "returning releases the loading surface");
                var retry = runtime.BeginRestartNewGame();
                presenter.Present(retry);
                Assert.That(presenter.Destination.environment, Is.EqualTo("night"));
                Assert.That(presenter.Destination.definition_hash,
                    Is.EqualTo(retry.pending_definition.definition_hash));
                Assert.That(presenter.IsVisible, Is.True);
                presenter.Present(GameRuntimeTransport.CancelLoading(retry));
                Assert.That(presenter.IsVisible, Is.False);
                Assert.That(runtime.IdentitySnapshot().mode, Is.EqualTo(GameRuntimeTransport.TitleMode));

            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
            }
        }

        [Test]
        public void DefinitionBoundCuesAcceptNewDestinationsAndRejectMismatches()
        {
            // A supplemental binding-validation check, not an installed-mission
            // or secret-playback claim. No closed list of level/cue IDs is used.
            var parent = new GameObject("definition-bound loading validation");
            try
            {
                using var presenter = new CampaignTransitionPresenter(parent.transform,
                    new byte[CampaignTransitionPresenter.FrameWidth * CampaignTransitionPresenter.FrameHeight],
                    new byte[256 * 3]);
                var identity = new GameRuntimeTransport.Identity {
                    mode = GameRuntimeTransport.LoadingMode, runtime_id = "7", generation = "2",
                    presentation = new GameRuntimeTransport.RuntimePresentationCue {
                        cue_id = "presentation.authored.new_destination.loading",
                        phase = GameRuntimeTransport.LoadingPresentationPhase,
                        level_id = "level.authored.new_destination"
                    },
                    pending_definition = new GameRuntimeTransport.DefinitionIdentity {
                        definition_hash = "sha256:" + new string('a', 64), package_graph_hash = "sha256:" + new string('b', 64),
                        definition_revision = "1",
                        runtime_flow = new GameRuntimeTransport.RuntimeFlowDefinition {
                            bound_level_id = "level.authored.new_destination"
                        }
                    }
                };
                var binding = new GameRuntimeTransport.LoadingPresentationBinding {
                    schema = "mc2-loading-presentation/v1", level_id = identity.presentation.level_id,
                    cue_id = identity.presentation.cue_id, definition_revision = "1",
                    definition_hash = identity.pending_definition.definition_hash,
                    package_graph_hash = identity.pending_definition.package_graph_hash,
                    source_package_sha256 = new string('c', 64), owner_export_package_sha256 = new string('d', 64),
                    runtime_capture_sha256 = new string('f', 64),
                    environment = "cave", artwork_profile = "ordinary"
                };
                presenter.Present(identity, binding);
                Assert.That(presenter.IsVisible, Is.True);
                binding.definition_hash = "sha256:" + new string('e', 64);
                Assert.Throws<InvalidDataException>(() => presenter.Present(identity, binding));
                Assert.That(presenter.IsVisible, Is.False, "a failed binding hides the previous frame");
                Assert.Throws<InvalidOperationException>(() =>
                    GameRuntimeTransport.ValidatePresentationCue(new GameRuntimeTransport.RuntimePresentationCue {
                        cue_id = identity.presentation.cue_id, phase = "invented", level_id = binding.level_id
                    }));
            }
            finally { UnityEngine.Object.DestroyImmediate(parent); }
        }

        [Test]
        [Timeout(180000)]
        public void MissingLoadingAssetsRetainRealDestinationThenRetryAndOpenOnce()
        {
            var directory = Path.Combine(Path.GetTempPath(), "mc-paj5-loading-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(directory);
            var parent = new GameObject("real loading retry");
            try
            {
                using var presenter = new CampaignTransitionPresenter(parent.transform, directory);
                using var runtime = new GameRuntimeTransport();
                var loading = runtime.BootstrapPackageSet(Package(LevelOnePackage, LevelOneAssetCache),
                    Package(LevelTwoPackage), Package(LevelThreePackage));
                var error = Assert.Catch<IOException>(() => presenter.Present(loading));
                presenter.Failed(error);
                Assert.That(presenter.HasFailure, Is.True);
                Assert.That(presenter.IsVisible, Is.False);
                var retained = runtime.IdentitySnapshot();
                Assert.That(retained.mode, Is.EqualTo(GameRuntimeTransport.LoadingMode));
                Assert.That(retained.generation, Is.EqualTo(loading.generation));
                Assert.That(retained.pending_definition.definition_hash, Is.EqualTo(loading.pending_definition.definition_hash));
                Assert.That(retained.session?.session_id, Is.Null.Or.Empty);

                var source = Path.Combine(PrivateRoot, "generated/mc2-loading-asset-cache");
                foreach (var file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
                {
                    var destination = Path.Combine(directory, Path.GetRelativePath(source, file));
                    Directory.CreateDirectory(Path.GetDirectoryName(destination));
                    File.Copy(file, destination);
                }
                presenter.Present(retained);
                Assert.That(presenter.HasFailure, Is.False);
                Assert.That(presenter.IndicesSha256, Is.EqualTo(ExpectedIndicesSha256));
                var gate = new CampaignTransitionPresenter.LoadingGate();
                gate.Begin(retained, presenter.Destination.level_id);
                Assert.That(gate.TryClaimOpen(retained, presenter.IsVisible), Is.False);
                gate.Observe(retained, presenter.IsVisible);
                Assert.That(gate.TryClaimOpen(retained, presenter.IsVisible), Is.True);
                var playing = runtime.OpenLevel();
                gate.Complete(playing);
                presenter.Present(playing);
                Assert.That(playing.session.tick, Is.EqualTo("0"));
                Assert.That(presenter.IsVisible, Is.False);
                var duplicate = Assert.Throws<GameRuntimeTransport.TransitionException>(() => runtime.OpenLevel());
                Assert.That(duplicate.Status, Is.EqualTo(3));
                Assert.That(duplicate.Message, Does.Contain("INVALID_RUNTIME_MODE"));
                Assert.That(runtime.IdentitySnapshot().session.generation, Is.EqualTo(playing.session.generation));
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
                // Only this test's newly created temporary copy is disposable.
                Directory.Delete(directory, true);
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
