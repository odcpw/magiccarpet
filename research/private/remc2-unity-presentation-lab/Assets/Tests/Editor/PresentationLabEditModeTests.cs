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
        public void DiagnosticOverlayDefaultsOffForValidatedOwnerModesAndIsOptIn()
        {
            OwnerInputValidation valid = OwnerInputValidation.Valid();
            OwnerInputValidation invalid = OwnerInputValidation.Invalid("missing owner inputs");

            Assert.IsFalse(PresentationModeRouter.ShouldShowDiagnostics(
                PresentationModeId.ClassicHd, valid, false));
            Assert.IsFalse(PresentationModeRouter.ShouldShowDiagnostics(
                PresentationModeId.UnityEnhanced, valid, false));
            Assert.IsTrue(PresentationModeRouter.ShouldShowDiagnostics(
                PresentationModeId.UnityEnhanced, valid, true));
            Assert.IsTrue(PresentationModeRouter.ShouldShowDiagnostics(
                PresentationModeId.Unknown, invalid, false));
            Assert.IsTrue(PresentationModeRouter.ShouldShowDiagnostics(
                PresentationModeId.ClassicHd, invalid, false));
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
            GameObject midView = new GameObject("mid-town-view");
            GameObject farView = new GameObject("far-town-view");
            nearView.transform.SetParent(root.transform, false);
            midView.transform.SetParent(root.transform, false);
            farView.transform.SetParent(root.transform, false);
            try
            {
                UnityEnhancedActorPresentation actors =
                    root.AddComponent<UnityEnhancedActorPresentation>();
                UnityEnhancedCameraPose cameraPose =
                    new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f);
                UnityEnhancedEntitySnapshot nearTown = TownEntity(10f);
                UnityEnhancedEntitySnapshot midTown = TownEntity(24f);
                UnityEnhancedEntitySnapshot farTown = TownEntity(60f);

                actors.Configure(nearView, nearTown, cameraPose);
                actors.Configure(midView, midTown, cameraPose);
                actors.Configure(farView, farTown, cameraPose);

                Transform nearVisual = nearView.transform.Find("EnhancedActorVisual");
                Transform midVisual = midView.transform.Find("EnhancedActorVisual");
                Transform farVisual = farView.transform.Find("EnhancedActorVisual");
                Assert.IsNotNull(nearVisual);
                Assert.IsNotNull(midVisual);
                Assert.IsNotNull(farVisual);
                Assert.That(nearVisual.localScale.y, Is.EqualTo(1.02f).Within(0.0001f));
                Assert.That(midVisual.localScale.y / nearVisual.localScale.y,
                    Is.EqualTo(1.25f).Within(0.01f));
                Assert.Greater(farVisual.localScale.y, midVisual.localScale.y);
                Assert.LessOrEqual(
                    farVisual.localScale.y,
                    nearVisual.localScale.y * 2.25f + 0.0001f);

                MeshRenderer nearRenderer = nearVisual.GetComponent<MeshRenderer>();
                MeshRenderer midRenderer = midVisual.GetComponent<MeshRenderer>();
                MeshRenderer farRenderer = farVisual.GetComponent<MeshRenderer>();
                MeshFilter nearFilter = nearVisual.GetComponent<MeshFilter>();
                MeshFilter midFilter = midVisual.GetComponent<MeshFilter>();
                MeshFilter farFilter = farVisual.GetComponent<MeshFilter>();
                Assert.AreSame(nearFilter.sharedMesh, farFilter.sharedMesh);
                Assert.AreSame(nearFilter.sharedMesh, midFilter.sharedMesh);
                Assert.AreSame(nearRenderer.sharedMaterial, farRenderer.sharedMaterial);
                Assert.AreSame(nearRenderer.sharedMaterial, midRenderer.sharedMaterial);
            }
            finally
            {
                Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void TownStructureAccentsExposeBoundedNightShimmer()
        {
            GameObject root = new GameObject("enhanced-town-accent-shimmer-test");
            GameObject townView = new GameObject("town-accent-view");
            GameObject castleView = new GameObject("castle-accent-view");
            townView.transform.SetParent(root.transform, false);
            castleView.transform.SetParent(root.transform, false);
            try
            {
                UnityEnhancedActorPresentation actors =
                    root.AddComponent<UnityEnhancedActorPresentation>();
                UnityEnhancedCameraPose cameraPose =
                    new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f);
                actors.Configure(townView, TownEntity(24f), cameraPose);
                actors.Configure(
                    castleView,
                    new UnityEnhancedEntitySnapshot
                    {
                        Generation = 1,
                        StableIdentity = 43,
                        Position = new Vector3(2f, 0f, 24f),
                        Rotation = Quaternion.identity,
                        Visible = true,
                        PresentationFamily = (int)UnityEnhancedPresentationFamily.Castle,
                        Scale = 1f,
                        RenderLayer = 7
                    },
                    cameraPose);

                Material townMaterial = townView
                    .transform.Find("EnhancedActorVisual")
                    .GetComponent<MeshRenderer>()
                    .sharedMaterial;
                Material castleMaterial = castleView
                    .transform.Find("EnhancedActorVisual")
                    .GetComponent<MeshRenderer>()
                    .sharedMaterial;
                Assert.IsNotNull(townMaterial);
                Assert.IsNotNull(castleMaterial);
                Assert.AreEqual(0.12f, townMaterial.GetFloat("_AccentShimmerStrength"), 0.001f);
                Assert.AreEqual(0f, castleMaterial.GetFloat("_AccentShimmerStrength"), 0.001f);
                Assert.LessOrEqual(townMaterial.GetFloat("_AccentShimmerStrength"), 0.12f);

                string source = File.ReadAllText(Path.Combine(
                    Application.dataPath,
                    "Resources",
                    "EnhancedVertexColorLit.shader"));
                StringAssert.Contains("_AccentShimmerStrength", source);
                StringAssert.Contains("_Time.y * 1.7h", source);
                StringAssert.Contains("accentMask * max(0.0h, accentShimmer)", source);
            }
            finally
            {
                Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void EnhancedTerrainSlopeReliefUsesBoundedAmbientFloor()
        {
            Shader shader = Shader.Find("MagicCarpet/Enhanced/Terrain");
            Assert.IsNotNull(shader);
            Material material = new Material(shader);
            try
            {
                float slopeStrength = material.GetFloat("_SlopeStrength");
                Assert.GreaterOrEqual(slopeStrength, 0f);
                Assert.LessOrEqual(slopeStrength, 1f);
                Assert.LessOrEqual(
                    slopeStrength * 0.28f,
                    0.28f + 0.0001f);

                string source = File.ReadAllText(Path.Combine(
                    Application.dataPath,
                    "Resources",
                    "EnhancedTerrain.shader"));
                StringAssert.Contains(
                    "half reliefFace = max(",
                    source);
                StringAssert.Contains(
                    "half ambientRelief = saturate(reliefFace * _SlopeStrength * 0.28h);",
                    source);
                StringAssert.Contains(
                    "half3 ambientFloor = _AmbientBoost * half3(0.60h, 0.73h, 0.96h) *",
                    source);
                StringAssert.Contains(
                    "SampleSH(normalWS) * (1.0h - ambientRelief)",
                    source);
                StringAssert.Contains(
                    "half3 baseColor = saturate(input.color.rgb * _BaseColor.rgb);",
                    source);
                StringAssert.Contains("float NearSurfaceFbm(float2 position)", source);
                StringAssert.Contains("half nearSurfaceFade = 1.0h - smoothstep(", source);
                StringAssert.Contains("_MicroReliefStrength * nearSurfaceFade", source);
                StringAssert.Contains("half3 microTint = lerp(", source);
                StringAssert.Contains(
                    "ApplyEnhancedAerialPerspective(color, input.positionWS)",
                    source);
            }
            finally
            {
                Object.DestroyImmediate(material);
            }
        }

        [Test]
        public void EnhancedWorldShadersExposeBoundedAerialPerspectiveContract()
        {
            string[] shaderNames =
            {
                "MagicCarpet/Enhanced/Terrain",
                "MagicCarpet/Enhanced/VertexColorLit",
                "MagicCarpet/Enhanced/Water"
            };
            string[] sourceNames =
            {
                "EnhancedTerrain.shader",
                "EnhancedVertexColorLit.shader",
                "EnhancedWater.shader"
            };

            for (int index = 0; index < shaderNames.Length; index++)
            {
                Shader shader = Shader.Find(shaderNames[index]);
                Assert.IsNotNull(shader, shaderNames[index]);
                Material material = new Material(shader);
                try
                {
                    Assert.AreEqual(48f, material.GetFloat("_EnhancedAerialStart"), 0.001f);
                    Assert.AreEqual(240f, material.GetFloat("_EnhancedAerialEnd"), 0.001f);
                    Assert.AreEqual(0.68f, material.GetFloat("_EnhancedAerialMaxBlend"), 0.001f);
                    Color aerialColor = material.GetColor("_EnhancedAerialColor");
                    Assert.That(aerialColor.r, Is.EqualTo(0.045f).Within(0.001f));
                    Assert.That(aerialColor.g, Is.EqualTo(0.12f).Within(0.001f));
                    Assert.That(aerialColor.b, Is.EqualTo(0.32f).Within(0.001f));
                    Assert.Less(material.GetFloat("_EnhancedAerialStart"),
                        material.GetFloat("_EnhancedAerialEnd"));
                    Assert.Less(material.GetFloat("_EnhancedAerialMaxBlend"), 1f);

                    string sourcePath = Path.Combine(
                        Application.dataPath,
                        "Resources",
                        sourceNames[index]);
                    string source = File.ReadAllText(sourcePath);
                    StringAssert.Contains("ApplyEnhancedAerialPerspective", source);
                    StringAssert.Contains(
                        "ApplyEnhancedAerialPerspective(color, input.positionWS)",
                        source);
                }
                finally
                {
                    Object.DestroyImmediate(material);
                }
            }
        }

        [Test]
        public void EnhancedNightSkyHorizonGlowIsBoundedAndApplied()
        {
            GameObject root = new GameObject("enhanced-night-sky-horizon-glow-test");
            Camera camera = root.AddComponent<Camera>();
            UnityEnhancedNightLighting lighting =
                root.AddComponent<UnityEnhancedNightLighting>();
            Material skyboxBefore = RenderSettings.skybox;
            try
            {
                lighting.ApplyDescriptor(
                    21,
                    UnityEnhancedNightLightingDescriptor.Level1Night(),
                    new UnityEnhancedLocalLightRequest[0],
                    true,
                    camera);

                Shader skyShader = Shader.Find("MagicCarpet/Enhanced/NightSky");
                Assert.IsNotNull(skyShader);
                Assert.IsNotNull(RenderSettings.skybox);
                Assert.AreEqual(skyShader, RenderSettings.skybox.shader);
                float strength = RenderSettings.skybox.GetFloat("_HorizonGlowStrength");
                Assert.Greater(strength, 0f);
                Assert.LessOrEqual(strength, 0.18f);

                string source = File.ReadAllText(Path.Combine(
                    Application.dataPath,
                    "Resources",
                    "EnhancedNightSky.shader"));
                StringAssert.Contains("HorizonGlowBand", source);
                StringAssert.Contains("half3 color = SkyGradient(direction);", source);
                StringAssert.Contains(
                    "half3 glowColor = saturate(",
                    source);
                StringAssert.Contains(
                    "color += glowColor * horizonBand * _HorizonGlowStrength;",
                    source);
            }
            finally
            {
                lighting.ResetForSession();
                Assert.AreSame(skyboxBefore, RenderSettings.skybox);
                Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void EnhancedNightSkyStarsUseBoundedTemporalTwinkle()
        {
            GameObject root = new GameObject("enhanced-night-sky-star-twinkle-test");
            Camera camera = root.AddComponent<Camera>();
            UnityEnhancedNightLighting lighting =
                root.AddComponent<UnityEnhancedNightLighting>();
            Material skyboxBefore = RenderSettings.skybox;
            try
            {
                lighting.ApplyDescriptor(
                    21,
                    UnityEnhancedNightLightingDescriptor.Level1Night(),
                    new UnityEnhancedLocalLightRequest[0],
                    true,
                    camera);

                Shader skyShader = Shader.Find("MagicCarpet/Enhanced/NightSky");
                Assert.IsNotNull(skyShader);
                Assert.IsNotNull(RenderSettings.skybox);
                Assert.AreEqual(skyShader, RenderSettings.skybox.shader);
                float strength = RenderSettings.skybox.GetFloat("_StarTwinkleStrength");
                Assert.Greater(strength, 0f);
                Assert.LessOrEqual(strength, 0.20f);

                string source = File.ReadAllText(Path.Combine(
                    Application.dataPath,
                    "Resources",
                    "EnhancedNightSky.shader"));
                StringAssert.Contains("float starSeed = Hash21(cell);", source);
                StringAssert.Contains(
                    "half twinkle = 1.0h + _StarTwinkleStrength * sin(",
                    source);
                StringAssert.Contains("_Time.y * (1.1h + starSeed * 1.9h)", source);
                StringAssert.Contains(
                    "half twinkleHalo = smoothstep(0.30h, 0.0h, length(local)) *",
                    source);
                StringAssert.Contains(
                    "candidate * (radius + twinkleHalo) * aboveHorizon *",
                    source);
            }
            finally
            {
                lighting.ResetForSession();
                Assert.AreSame(skyboxBefore, RenderSettings.skybox);
                Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void EnhancedNightSkyStarsReachOwnerFlightHorizonWithoutCrossingIt()
        {
            GameObject root = new GameObject("enhanced-night-sky-owner-horizon-test");
            Camera camera = root.AddComponent<Camera>();
            UnityEnhancedNightLighting lighting =
                root.AddComponent<UnityEnhancedNightLighting>();
            Material skyboxBefore = RenderSettings.skybox;
            try
            {
                lighting.ApplyDescriptor(
                    22,
                    UnityEnhancedNightLightingDescriptor.Level1Night(),
                    new UnityEnhancedLocalLightRequest[0],
                    true,
                    camera);

                Shader skyShader = Shader.Find("MagicCarpet/Enhanced/NightSky");
                Assert.IsNotNull(skyShader);
                Assert.IsNotNull(RenderSettings.skybox);
                Assert.AreEqual(skyShader, RenderSettings.skybox.shader);
                Assert.That(RenderSettings.skybox.GetFloat("_StarDensity"),
                    Is.EqualTo(0.025f).Within(0.0001f));
                Assert.That(RenderSettings.skybox.GetFloat("_StarIntensity"),
                    Is.EqualTo(0.8f).Within(0.0001f));
                Assert.That(RenderSettings.skybox.GetFloat("_StarTwinkleStrength"),
                    Is.EqualTo(0.18f).Within(0.0001f));

                string source = File.ReadAllText(Path.Combine(
                    Application.dataPath,
                    "Resources",
                    "EnhancedNightSky.shader"));
                StringAssert.Contains(
                    "half aboveHorizon = smoothstep(0.015h, 0.14h, direction.y);",
                    source);
                StringAssert.Contains(
                    "candidate * (radius + twinkleHalo) * aboveHorizon *",
                    source);
            }
            finally
            {
                lighting.ResetForSession();
                Assert.AreSame(skyboxBefore, RenderSettings.skybox);
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
