#if UNITY_EDITOR
using System.IO;
using NUnit.Framework;

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
    }
}
#endif
