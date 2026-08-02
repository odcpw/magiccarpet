#if UNITY_EDITOR
using System.IO;
using NUnit.Framework;
using UnityEngine;

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
    }
}
#endif
