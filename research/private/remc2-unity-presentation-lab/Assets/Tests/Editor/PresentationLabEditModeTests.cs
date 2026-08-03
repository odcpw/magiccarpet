#if UNITY_EDITOR
using System.IO;
using NUnit.Framework;
using UnityEngine;
using MagicCarpet.PresentationLab.Editor;

namespace MagicCarpet.PresentationLab.Tests
{
    public sealed class PresentationLabEditModeTests
    {
        [Test]
        public void MissingOwnerInputsFailClosed()
        {
            OwnerInputValidation validation = OwnerInputValidator.Validate(
                PresentationModeId.UnityEnhanced,
                string.Empty,
                string.Empty,
                string.Empty,
                "/tmp/presentation-lab");

            Assert.IsFalse(validation.IsValid);
            StringAssert.Contains("missing required owner input", validation.Reason);
        }

        [Test]
        public void ProjectInternalOwnerInputsAreRejectedBeforeLoading()
        {
            string projectRoot = "/tmp/presentation-lab";
            string packagePath = Path.Combine(projectRoot, "Assets", "owner-package");
            OwnerInputValidation validation = OwnerInputValidator.Validate(
                PresentationModeId.ClassicHd,
                packagePath,
                "/tmp/closure.json",
                "/tmp/owner-cache",
                projectRoot);

            Assert.IsFalse(validation.IsValid);
            StringAssert.Contains("outside the presentation-lab project", validation.Reason);
        }

        [Test]
        public void SessionLeaseHasExplicitOpenAndCloseLifecycle()
        {
            using (PresentationSessionLease lease = PresentationSessionLease.Open(
                       PresentationModeId.UnityEnhanced, OwnerInputValidation.Valid()))
            {
                Assert.IsTrue(lease.IsOpen);
                Assert.AreEqual(PresentationModeId.UnityEnhanced, lease.Mode);
            }
        }

        [Test]
        public void UnsupportedModeCannotOpenSession()
        {
            Assert.Throws<System.InvalidOperationException>(() => PresentationSessionLease.Open(
                PresentationModeId.Unknown, OwnerInputValidation.Valid()));
        }

        [Test]
        public void ClassicHdDefaultsSelectMandatory640By480OriginalTier()
        {
            ClassicHdPresentationSettings settings = ClassicHdPresentationSettings.Default();
            string reason;
            Assert.IsTrue(settings.TryValidate(out reason), reason);
            Assert.AreEqual("hd-640x480", settings.profile);
            Assert.AreEqual("original-indexed", settings.assetTier);
            ClassicHdProfileInfo profile;
            Assert.IsTrue(ClassicHdPresentationSettings.TryGetProfile(settings.profile, out profile));
            Assert.AreEqual(640, profile.Width);
            Assert.AreEqual(480, profile.Height);
        }

        [Test]
        public void ClassicHdBlurDefaultsOffAndExplicitTableSnRemainsAccepted()
        {
            ClassicHdPresentationSettings settings = ClassicHdPresentationSettings.Default();
            string reason;
            Assert.IsTrue(settings.TryValidate(out reason), reason);
            Assert.AreEqual("off", settings.blur);

            ClassicHdSettingsLoadResult result = ClassicHdSettingsStore.LoadFromArguments(new[]
            {
                "player",
                "-mc-classic-blur", "tablesn"
            });
            Assert.IsTrue(result.IsValid, result.Reason);
            Assert.AreEqual("tablesn", result.Settings.blur);
        }

        [Test]
        public void ClassicHdOptionalAssetTierFallsBackExplicitly()
        {
            ClassicHdProfileInfo profile;
            Assert.IsTrue(ClassicHdPresentationSettings.TryGetProfile("hd-960x720", out profile));

            ClassicHdAssetTierResolution resolution = ClassicHdAssetTierResolver.Resolve(
                profile,
                ClassicHdAssetTierResolver.OptionalHighResolutionToken,
                Path.Combine(Path.GetTempPath(), "magiccarpet-missing-owner-cache"));

            Assert.IsTrue(resolution.FallbackApplied);
            Assert.AreEqual(ClassicHdAssetTierResolver.OptionalHighResolutionToken, resolution.RequestedToken);
            Assert.AreEqual(ClassicHdAssetTierResolver.OriginalIndexedToken, resolution.EffectiveToken);
            StringAssert.Contains("absent", resolution.Reason);
        }

        [Test]
        public void ClassicHdOptionalAssetPackStillNeedsNativeQualification()
        {
            string cacheRoot = Path.Combine(Path.GetTempPath(), "magiccarpet-classic-hd-optional-pack-test");
            string optionalRoot = Path.Combine(cacheRoot, ClassicHdAssetTierResolver.OptionalPackDirectory);
            try
            {
                Directory.CreateDirectory(optionalRoot);
                File.WriteAllText(
                    Path.Combine(optionalRoot, ClassicHdAssetTierResolver.OptionalPackManifest),
                    "{\"schema_version\":1}");

                ClassicHdProfileInfo profile;
                Assert.IsTrue(ClassicHdPresentationSettings.TryGetProfile("hd-960x720", out profile));
                ClassicHdAssetTierResolution resolution = ClassicHdAssetTierResolver.Resolve(
                    profile,
                    ClassicHdAssetTierResolver.OptionalHighResolutionToken,
                    cacheRoot);

                Assert.IsTrue(resolution.OptionalPackPresent);
                Assert.IsTrue(resolution.FallbackApplied);
                StringAssert.Contains("not yet qualified", resolution.Reason);
            }
            finally
            {
                if (Directory.Exists(cacheRoot))
                {
                    Directory.Delete(cacheRoot, true);
                }
            }
        }

        [Test]
        public void ClassicHdPresentationLifecycleTracksFocusAndPause()
        {
            GameObject gameObject = new GameObject("classic-hd-lifecycle-test");
            ClassicHdPresentation presentation = gameObject.AddComponent<ClassicHdPresentation>();
            string reason;
            Assert.IsTrue(presentation.Configure(ClassicHdPresentationSettings.Default(), out reason), reason);
            Assert.IsFalse(presentation.IsApplicationSuspended);

            Assert.IsTrue(presentation.SetApplicationLifecycleState(false, false, out reason), reason);
            Assert.IsTrue(presentation.IsApplicationSuspended);
            Assert.IsTrue(presentation.SetApplicationLifecycleState(true, true, out reason), reason);
            Assert.IsTrue(presentation.IsApplicationSuspended);
            Assert.IsTrue(presentation.SetApplicationLifecycleState(true, false, out reason), reason);
            Assert.IsFalse(presentation.IsApplicationSuspended);
            Assert.AreEqual(3, presentation.ApplicationLifecycleTransitions);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ClassicHdSettingsRoundTripOutsideOwnerData()
        {
            string path = Path.Combine(Path.GetTempPath(), "magiccarpet-classic-hd-settings-test.json");
            try
            {
                ClassicHdPresentationSettings settings = ClassicHdPresentationSettings.Default();
                settings.profile = "hd-960x720";
                settings.reflections = false;
                settings.outputFilter = "bilinear";
                string reason;
                Assert.IsTrue(ClassicHdSettingsStore.TrySave(path, settings, out reason), reason);
                ClassicHdPresentationSettings loaded;
                Assert.IsTrue(ClassicHdSettingsStore.TryRead(path, out loaded, out reason), reason);
                Assert.AreEqual(settings.profile, loaded.profile);
                Assert.AreEqual(settings.reflections, loaded.reflections);
                Assert.AreEqual(settings.outputFilter, loaded.outputFilter);
            }
            finally
            {
                if (File.Exists(path))
                {
                    File.Delete(path);
                }
            }
        }

        [Test]
        public void InvalidClassicHdSettingsFailClosed()
        {
            string path = Path.Combine(Path.GetTempPath(), "magiccarpet-invalid-classic-hd-settings.json");
            try
            {
                File.WriteAllText(path, "{\"schemaVersion\":99}");
                ClassicHdSettingsLoadResult result = ClassicHdSettingsStore.LoadFromArguments(
                    new[] { "player", "-mc-settings", path });
                Assert.IsFalse(result.IsValid);
                StringAssert.Contains("schema version", result.Reason);
            }
            finally
            {
                if (File.Exists(path))
                {
                    File.Delete(path);
                }
            }
        }

        [Test]
        public void ClassicHdCommandLineOverridesRoundTrip()
        {
            ClassicHdSettingsLoadResult result = ClassicHdSettingsStore.LoadFromArguments(new[]
            {
                "player",
                "-mc-classic-resolution", "960x720",
                "-mc-classic-reflections", "off",
                "-mc-window-width", "1920",
                "-mc-window-height", "1080",
                "-mc-output-filter", "bilinear"
            });
            Assert.IsTrue(result.IsValid, result.Reason);
            Assert.AreEqual("hd-960x720", result.Settings.profile);
            Assert.IsFalse(result.Settings.reflections);
            Assert.AreEqual(1920, result.Settings.windowWidth);
            Assert.AreEqual(1080, result.Settings.windowHeight);
            Assert.AreEqual("bilinear", result.Settings.outputFilter);
        }

        [Test]
        public void ClassicHdDisplayIsCenteredAndKeepsFourByThree()
        {
            Rect viewport = ClassicHdDisplayComposition.CalculateViewport(1920, 1080, 640, 480);
            Assert.AreEqual(1440, Mathf.RoundToInt(viewport.width));
            Assert.AreEqual(1080, Mathf.RoundToInt(viewport.height));
            Assert.AreEqual(240, Mathf.RoundToInt(viewport.x));
            Assert.AreEqual(0, Mathf.RoundToInt(viewport.y));
        }

        [Test]
        public void ClassicHdAbiStructsMatchRustLayout()
        {
            string reason;
            Assert.IsTrue(ClassicHdAbiContract.HasExpectedLayout(out reason), reason);
            Assert.AreEqual(1u, ClassicHdAbiContract.AbiVersion);
            Assert.AreEqual(4u, ClassicHdAbiContract.ProfileCount);
        }

        [Test]
        public void OwnerCommandJournalReadsTheSealedLittleEndianRecord()
        {
            string root = Path.Combine(
                Path.GetTempPath(),
                "mc2-command-journal-" + System.Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(root);
            try
            {
                File.WriteAllText(
                    Path.Combine(root, "manifest.json"),
                    "{\"schema\":\"mc2-unity-playthrough/v1\",\"status\":\"complete\"," +
                    "\"package_path\":\"/external/package\",\"closure_path\":\"/external/closure.json\"," +
                    "\"command_encoding\":\"mc-native-command-10-byte-le/v1\",\"command_bytes\":10," +
                    "\"command_count\":1}");
                File.WriteAllBytes(
                    Path.Combine(root, "commands.bin"),
                    new byte[] { 40, 7, 6, 0xFC, 0x80, 0x41, 0xFF, 0xFF, 0, 0 });

                ClassicHdOwnerCommandJournal journal;
                string reason;
                Assert.IsTrue(
                    ClassicHdOwnerCommandJournal.TryOpen(root, out journal, out reason),
                    reason);
                using (journal)
                {
                    Assert.IsTrue(
                        journal.MatchesExternalInputs(
                            "/external/package",
                            "/external/closure.json",
                            out reason),
                        reason);
                    Assert.IsFalse(
                        journal.MatchesExternalInputs(
                            "/external/other-package",
                            "/external/closure.json",
                            out reason),
                        "a journal bound to a different external package must fail closed");
                    ClassicHdOwnerCommand command;
                    Assert.IsTrue(journal.ReadNext(out command, out reason), reason);
                    Assert.AreEqual((sbyte)40, command.Opcode);
                    Assert.AreEqual((sbyte)7, command.Argument1);
                    Assert.AreEqual((sbyte)6, command.Argument2);
                    Assert.AreEqual((sbyte)-4, command.Roll);
                    Assert.AreEqual(sbyte.MinValue, command.Pitch);
                    Assert.AreEqual((sbyte)0x41, command.Flags);
                    Assert.AreEqual((short)-1, command.RequestedSpell);
                    Assert.AreEqual(1, journal.ReadCount);
                }
            }
            finally
            {
                if (Directory.Exists(root))
                {
                    Directory.Delete(root, true);
                }
            }
        }

        [Test]
        public void OwnerCommandJournalRejectsIncompleteRecorderOutput()
        {
            string root = Path.Combine(
                Path.GetTempPath(),
                "mc2-command-journal-incomplete-" + System.Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(root);
            try
            {
                File.WriteAllText(
                    Path.Combine(root, "manifest.json"),
                    "{\"schema\":\"mc2-unity-playthrough/v1\",\"status\":\"running\"," +
                    "\"package_path\":\"/external/package\",\"closure_path\":\"/external/closure.json\"," +
                    "\"command_encoding\":\"mc-native-command-10-byte-le/v1\",\"command_bytes\":10," +
                    "\"command_count\":0}");
                File.WriteAllBytes(Path.Combine(root, "commands.bin"), new byte[0]);

                ClassicHdOwnerCommandJournal journal;
                string reason;
                Assert.IsFalse(
                    ClassicHdOwnerCommandJournal.TryOpen(root, out journal, out reason));
                StringAssert.Contains("not complete", reason);
            }
            finally
            {
                if (Directory.Exists(root))
                {
                    Directory.Delete(root, true);
                }
            }
        }

        [Test]
        public void OwnerCommandJournalRejectsNonZeroRecorderReservedWords()
        {
            string root = Path.Combine(
                Path.GetTempPath(),
                "mc2-command-journal-reserved-" + System.Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(root);
            try
            {
                File.WriteAllText(
                    Path.Combine(root, "manifest.json"),
                    "{\"schema\":\"mc2-unity-playthrough/v1\",\"status\":\"complete\"," +
                    "\"package_path\":\"/external/package\",\"closure_path\":\"/external/closure.json\"," +
                    "\"command_encoding\":\"mc-native-command-10-byte-le/v1\",\"command_bytes\":10," +
                    "\"command_count\":1}");
                File.WriteAllBytes(
                    Path.Combine(root, "commands.bin"),
                    new byte[] { 0, 0, 0, 0, 0, 1, 0xFF, 0xFF, 1, 0 });

                ClassicHdOwnerCommandJournal journal;
                string reason;
                Assert.IsTrue(
                    ClassicHdOwnerCommandJournal.TryOpen(root, out journal, out reason),
                    reason);
                using (journal)
                {
                    ClassicHdOwnerCommand command;
                    Assert.IsFalse(journal.ReadNext(out command, out reason));
                    StringAssert.Contains("reserved bytes", reason);
                }
            }
            finally
            {
                if (Directory.Exists(root))
                {
                    Directory.Delete(root, true);
                }
            }
        }

        [Test]
        public void TownStructureReadabilityScaleIsDepthAwareAndBounded()
        {
            GameObject root = new GameObject("enhanced-town-depth-readability-test");
            GameObject nearView = new GameObject("near-town-view");
            GameObject farView = new GameObject("far-town-view");
            nearView.transform.SetParent(root.transform, false);
            farView.transform.SetParent(root.transform, false);
            try
            {
                UnityEnhancedActorPresentation actors =
                    root.AddComponent<UnityEnhancedActorPresentation>();
                UnityEnhancedCameraPose cameraPose =
                    new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f);
                UnityEnhancedEntitySnapshot nearTown = TownEntity(24f);
                UnityEnhancedEntitySnapshot farTown = TownEntity(60f);

                actors.Configure(nearView, nearTown, cameraPose);
                actors.Configure(farView, farTown, cameraPose);

                Transform nearVisual = nearView.transform.Find("EnhancedActorVisual");
                Transform farVisual = farView.transform.Find("EnhancedActorVisual");
                Assert.IsNotNull(nearVisual);
                Assert.IsNotNull(farVisual);
                Assert.Greater(farVisual.localScale.y, nearVisual.localScale.y);
                Assert.LessOrEqual(
                    farVisual.localScale.y,
                    nearVisual.localScale.y * 1.5f + 0.0001f);

                MeshRenderer nearRenderer = nearVisual.GetComponent<MeshRenderer>();
                MeshRenderer farRenderer = farVisual.GetComponent<MeshRenderer>();
                MeshFilter nearFilter = nearVisual.GetComponent<MeshFilter>();
                MeshFilter farFilter = farVisual.GetComponent<MeshFilter>();
                Assert.AreSame(nearFilter.sharedMesh, farFilter.sharedMesh);
                Assert.AreSame(nearRenderer.sharedMaterial, farRenderer.sharedMaterial);
            }
            finally
            {
                Object.DestroyImmediate(root);
            }
        }

        private static UnityEnhancedEntitySnapshot TownEntity(float forwardDepth)
        {
            return new UnityEnhancedEntitySnapshot
            {
                Generation = 1,
                StableIdentity = 42,
                Position = new Vector3(0f, 0f, forwardDepth),
                Rotation = Quaternion.identity,
                Visible = true,
                PresentationFamily = (int)UnityEnhancedPresentationFamily.Town,
                Scale = 1f,
                RenderLayer = 7
            };
        }

    }
}
#endif
