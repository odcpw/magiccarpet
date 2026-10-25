using System;
using System.IO;
using System.Collections.Generic;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class CampaignMinimapPixelTests
    {
        [Serializable]
        private sealed class CacheIdentity { public string cache_content_sha256; }

        private static void AssertNativeSpriteJoin(MagicCarpetRuntime runtime, CampaignPresentationSurface surface)
        {
            var campaign = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField("campaignSession",
                System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
            var snapshots = campaign.Entities();
            var yaw = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose.yaw;
            var draws = campaign.EntityPresentations(yaw);
            var markers = surface.Root.GetComponentsInChildren<Level3EntitySurfaceMarker>();
            var admitted = new HashSet<ushort>();
            foreach (var draw in draws)
            {
                Assert.That(snapshots[draw.entityIndex].active, Is.EqualTo(1),
                    $"drawn entity {draw.entityIndex} must be present in the live roster");
                var marker = Array.Find(markers, row => row.EntityIndex == draw.entityIndex);
                Assert.That(marker, Is.Not.Null, $"drawn entity {draw.entityIndex} must have a Unity marker");
                Assert.That(marker.SpriteReady, Is.True);
                Assert.That(marker.AssetSelector, Is.EqualTo(draw.selectedTmap));
                admitted.Add(draw.entityIndex);
            }
            foreach (var marker in markers)
                Assert.That(marker.SpriteReady, Is.EqualTo(admitted.Contains(marker.EntityIndex)),
                    $"entity {marker.EntityIndex} must follow this revision's source draw admission");
        }
        [TestCase(0)]
        [TestCase(326)]
        [TestCase(2418)]
        public void ProductionRootUsesPackageEnvironmentWithQualifiedHudCompanion(int checkpointTick)
        {
            var restoreEarnedOpening = checkpointTick != 0;
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var root = new GameObject("package environment production root");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var night = Path.Combine(generated, restoreEarnedOpening
                ? "mc2-level1-night-cache-v2" : "level1-unity-asset-cache-plan29-final");
            var environment = Path.Combine(generated, "mc-oi62-environment-level1-v2");
            var arguments = new List<string> {
                "-mc-game-runtime", "-mc-runtime-loading-cache", night,
                "-mc-runtime-package", Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level1-v2"),
                "-mc-runtime-presentation-cache", night,
                "-mc-runtime-package", Path.Combine(generated, "level2-day-block32-campaign-v2"),
                "-mc-runtime-owner-cache", Path.Combine(generated, "mc-oi62-environment-level2-v2"),
                "-mc-runtime-presentation-cache", Path.Combine(generated, "level2-day-spell-owner-cache-v3"),
                "-mc-runtime-package", Path.Combine(generated, "level3-day-block32-campaign-recipe-v2")
            };
            if (restoreEarnedOpening)
            {
                arguments[arguments.IndexOf("-mc-runtime-owner-cache") + 1] = night;
                var dayOwner = arguments.IndexOf("-mc-runtime-owner-cache",
                    arguments.IndexOf("-mc-runtime-owner-cache") + 1);
                arguments[dayOwner + 1] = Path.Combine(generated, "level2-day-spell-owner-cache-v3/components/material");
                arguments.AddRange(new[] {
                    "-mc-runtime-owner-cache", Path.Combine(generated, "level3-day-spell-owner-cache-v3/components/material"),
                    "-mc-runtime-presentation-cache", Path.Combine(generated, "level3-day-spell-owner-cache-v3")
                });
            }
            if (restoreEarnedOpening) arguments.AddRange(new[] { "-mc-runtime-replay-load",
                Path.Combine(generated, checkpointTick == 326
                    ? "mc-a1-opening.Lcb3s1/native-beacon-branch.json"
                    : "mc-a1-opening.Lcb3s1/earned-progress.json") });
            else arguments.Add("-mc-runtime-menu");
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                if (!restoreEarnedOpening)
                {
                    Assert.That(runtime.ExecuteGameRuntimeMenuAction(CampaignMenuPresenter.Action.NewGame), Is.True);
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    runtime.QualificationObserveGameRuntimeLoadingPresentationFrame();
                    runtime.QualificationDriveGameRuntimeFrames(1);
                }
                Assert.That(runtime.GameRuntimeProductionIdentity.mode, Is.EqualTo(GameRuntimeTransport.PlayingMode));
                var surface = (CampaignPresentationSurface)typeof(MagicCarpetRuntime).GetField("gameRuntimeNightSurface",
                    System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
                Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                var expected = OwnerAssetCache.OpenAndValidateEnvironment(environment,
                    Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"));
                Assert.That(surface.CacheContentHash, Is.EqualTo(restoreEarnedOpening
                        ? JsonUtility.FromJson<CacheIdentity>(File.ReadAllText(Path.Combine(night, "manifest.json"))).cache_content_sha256
                        : expected.ContentHash),
                    "the production surface consumes the package-selected products");
                if (checkpointTick == 326)
                {
                    Assert.That(surface.Tick, Is.EqualTo(326ul));
                    var marker = Array.Find(surface.Root.GetComponentsInChildren<Level3EntitySurfaceMarker>(),
                        row => row.EntityIndex == 122);
                    Assert.That(marker, Is.Not.Null, "the actual earned beacon draw must have its live snapshot");
                    Assert.That(marker.SpriteReady, Is.True);
                    Assert.That(marker.AssetSelector, Is.EqualTo(57), "the genuine earned326 draw that failed on installed2c");
                }
                else if (checkpointTick == 2418)
                {
                    Assert.That(surface.Tick, Is.EqualTo(2418ul));
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    Assert.That(surface.Tick, Is.EqualTo(2419ul), "ordinary first turn after the real restore");
                    var marker = Array.Find(surface.Root.GetComponentsInChildren<Level3EntitySurfaceMarker>(),
                        row => row.EntityIndex == 151);
                    Assert.That(marker, Is.Not.Null, "actual saved actor must reach the ordinary presentation consumer");
                    // The exact native probe establishes a class11 trigger at
                    // saved2418/neutral2419. The original fatal followed C/J/C/
                    // forward input at an unknown later tick, not this turn.
                    Assert.That(marker.ClassId, Is.EqualTo(11));
                    Assert.That(marker.Model, Is.Zero);
                }
                AssertNativeSpriteJoin(runtime, surface);
                Assert.That(runtime.Level2ProductionDayHudPresenterReady, Is.True,
                    "qualified HUD companion remains live alongside the selected world materials");
                var before = surface.FrameHash;
                runtime.QualificationApplyGameRuntimeProductionMovement(true, false, true, false);
                runtime.QualificationApplyGameRuntimeProductionSteering(24f, -8f);
                for (var turn = 0; turn < 3; turn++)
                {
                    runtime.QualificationDriveGameRuntimeFrames(1);
                    AssertNativeSpriteJoin(runtime, surface);
                }
                Assert.That(surface.FrameHash, Is.Not.EqualTo(before));
                Assert.That(surface.Generation, Is.EqualTo(ulong.Parse(runtime.GameRuntimeProductionIdentity.session.generation)));
            }
            finally
            {
                runtime.QualificationDisposeGameRuntimeProduction();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void PackageEnvironmentMaterialsReachRealNightAndDaySurfaces()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            foreach (var level in new[] { 1, 2 })
            {
                var package = Path.Combine(generated, level == 1 ? "mc-bhq8-level1-collision-v5-night" : "level2-day-block32-campaign-v2");
                var cachePath = Path.Combine(generated, $"mc-oi62-environment-level{level}-v2");
                var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
                var selection = NativeSim.CampaignSelection.Create((ushort)level, (ushort)(level - 1), (byte)(level == 1 ? 1 : 0));
                using (var session = NativeCampaignSession.Open(package, selection, cachePath))
                using (var surface = new CampaignPresentationSurface())
                {
                    surface.PresentEnvironment(session, cache);
                    Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                    Assert.That(surface.CacheContentHash, Is.EqualTo(cache.ContentHash));
                    Assert.That(surface.Generation, Is.EqualTo(session.Generation));
                    Assert.That(surface.FrameHash, Is.Not.Zero);
                    // A second poll observes the same authoritative revision.
                    var first = surface.FrameHash;
                    surface.PresentEnvironment(session, cache);
                    Assert.That(surface.FrameHash, Is.EqualTo(first));
                    var wrong = OwnerAssetCache.OpenAndValidateEnvironment(
                        Path.Combine(generated, $"mc-oi62-environment-level{(level == 1 ? 2 : 1)}-v2"),
                        Path.Combine(generated, level == 1 ? "level2-day-block32-campaign-v2" : "mc-bhq8-level1-collision-v5-night"));
                    Assert.Throws<InvalidOperationException>(() => surface.PresentEnvironment(session, wrong));
                    Assert.That(surface.FrameHash, Is.EqualTo(first));
                    Assert.That(surface.WorldReady, Is.True, "wrong environment preserves current visible scene");
                }
            }
        }

        [Test]
        public void RealCaveAndFireNightResourcesUsePackageSelectors()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            foreach (var level in new[] { 4, 25 })
            {
                var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                    Path.Combine(generated, $"mc-oi62-environment-level{level}-v2"),
                    Path.Combine(generated, $"mc-bhq6-ordinary-20261025-zclu5fjj/level{level}"));
                var materials = cache.EnvironmentMaterials;
                Assert.That(materials.profile.external_level, Is.EqualTo(level));
                Assert.That(materials.profile.palette, Is.EqualTo(level == 4 ? "PALC-0" : "PALF-0"));
                Assert.That(materials.sky == null, Is.EqualTo(level == 4));
                Assert.That(materials.tmaps.textures.Length, Is.EqualTo(504));
            }
        }

        [Test]
        public void DaySpriteProjectionUsesSourcePitchOffsetAndPixelSpaceRoll()
        {
            // Source projected point: x=164*256/1024=41;
            // y=(32*320>>8)+164*128/1024=60.5, measured upward.
            var point = new Vector4(256f / 64f, 128f / 64f, -1024f / 64f, 1f);
            var clip = CampaignPresentationSurface.SourceSpriteProjection(32, 0, .035f, 1800f) * point;
            Assert.That((clip.x / clip.w + 1f) * 160f, Is.EqualTo(201f).Within(.001f));
            Assert.That((1f - clip.y / clip.w) * 100f, Is.EqualTo(39.5f).Within(.001f));
            var rolled = CampaignPresentationSurface.SourceSpriteProjection(32, 512, .035f, 1800f) * point;
            Assert.That((rolled.x / rolled.w + 1f) * 160f, Is.EqualTo(99.5f).Within(.001f));
            Assert.That((1f - rolled.y / rolled.w) * 100f, Is.EqualTo(59f).Within(.001f));
            var farther = new Vector4(point.x * 2, point.y * 2, point.z * 2, 1f);
            var distant = CampaignPresentationSurface.SourceSpriteProjection(32, 0, .035f, 1800f) * farther;
            Assert.That(distant.y / distant.w, Is.EqualTo(clip.y / clip.w).Within(.00001f),
                "pitch is a depth-independent screen offset, not world rotation");
        }

        [Test]
        public void CorrectedDayReplayUsesMaterialCacheWithoutChangingGameplay()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var package = Path.Combine(generated, "level2-day-block32-campaign-v2");
            var cachePath = Path.Combine(generated, "level2-day-minimap-owner-cache-v3");
            var packages = new[] {
                new GameRuntimeTransport.InstalledPackageResource {
                    package_dir = package,
                    owner_cache_path = Path.Combine(cachePath, "components/material") },
                new GameRuntimeTransport.InstalledPackageResource {
                    package_dir = Path.Combine(generated, "mc-cf15-level1-package-v3-current") },
                new GameRuntimeTransport.InstalledPackageResource {
                    package_dir = Path.Combine(generated, "level3-day-block32-campaign-recipe-v2") }
            };
            var replay = File.ReadAllBytes(Path.Combine(generated, "opened-l2-block32-replay-v5.json"));
            var root = new GameObject("Day runtime replay test");
            try
            {
                using (var runtime = new GameRuntimeTransport())
                {
                    var identity = runtime.LoadReplay(replay, packages);
                    using (var session = NativeCampaignSession.BorrowGameRuntime(
                        runtime.Handle, identity, package, NativeSim.CampaignSelection.Level2Day()))
                    using (var surface = new CampaignPresentationSurface(root.transform))
                    {
                        surface.Present(session, OwnerAssetCache.OpenAndValidateLevel2(cachePath));
                        Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                        Assert.That(surface.RenderedEntitySurfaceCount, Is.Zero,
                            "this replay starts beyond the source sprite draw range");
                        Assert.That(surface.Tick, Is.EqualTo(1ul));
                    }
                }
                packages[0].owner_cache_path = Path.Combine(generated,
                    "level3-day-minimap-owner-cache-v3/components/material");
                using (var wrong = new GameRuntimeTransport())
                {
                    var error = Assert.Throws<GameRuntimeTransport.TransitionException>(
                        () => wrong.LoadReplay(replay, packages));
                    Assert.That(error.Status, Is.EqualTo(NativeSim.ValidationFailed));
                    Assert.That(error.Message, Does.Contain("Day material cache does not match"));
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(root); }
        }

        [Test]
        public void CorrectedLevel2PackageRendersThroughTheSharedDaySurface()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var cachePath = Path.Combine(generated, "level2-day-minimap-owner-cache-v3");
            var cache = OwnerAssetCache.OpenAndValidateLevel2(cachePath);
            var root = new GameObject("shared Day Level 2 test");
            CampaignPresentationSurface surface = null;
            try
            {
                using (var session = NativeCampaignSession.Open(
                    Path.Combine(generated, "level2-day-block32-campaign-v2"),
                    NativeSim.CampaignSelection.Create(2, 1, 0),
                    Path.Combine(cachePath, "components/material")))
                {
                    surface = new CampaignPresentationSurface(root.transform);
                    surface.Present(session, cache);
                    Assert.That(surface.WorldReady, Is.True);
                    Assert.That(surface.LiveMinimapReady, Is.True);
                    Assert.That(surface.RenderedEntitySurfaceCount, Is.Zero,
                        "the starting sea view must not draw distant inland actors over the sky");
                    Assert.That((surface.Generation, surface.Tick), Is.EqualTo((0ul, 0ul)));
                    var actor = Array.Find(session.Entities(), row => row.active != 0 && row.classId == 5);
                    Assert.That(actor.entityIndex, Is.GreaterThan(0), "real package creature exists");
                    // Explicit camera placement through the existing native test
                    // transport; this is a near-view render probe, not flight proof.
                    session.Step(actor.x, unchecked((ushort)(actor.y + 1024)), actor.z);
                    surface.Present(session, cache);
                    Assert.That(surface.RenderedEntitySurfaceCount, Is.GreaterThan(0),
                        "native admission still presents nearby real actors");
                }
            }
            finally
            {
                surface?.Dispose();
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [Test]
        public void BothDayPackagesLoadOriginalSymbolsWithoutCrossLevelSubstitution()
        {
            var level2 = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../generated/level2-day-minimap-owner-cache-v3"));
            var level3 = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../generated/level3-day-minimap-owner-cache-v3"));
            var cache = OwnerAssetCache.OpenAndValidateLevel2(level2);
            Assert.That(cache.Level3Materials.profile.external_level, Is.EqualTo(2));
            Assert.That(cache.Level3Materials.tmaps0.textures.Length, Is.EqualTo(156));
            Assert.That(cache.Level3Materials.minimap_symbols.symbols.Length, Is.EqualTo(16));
            Assert.That(OwnerAssetCache.OpenAndValidateLevel3(level3).Level3Materials.profile.external_level, Is.EqualTo(3));
            Assert.Throws<InvalidDataException>(() => OwnerAssetCache.OpenAndValidateLevel2(level3));
            Assert.Throws<InvalidDataException>(() => OwnerAssetCache.OpenAndValidateLevel3(level2));
        }

        [Test]
        public void NativeArrowDrawsClosedOutlineAndRejectsPartialOrStaleVertices()
        {
            var pixels = new byte[320 * 200];
            var tables = new byte[83456];
            var xy = new short[] { 64, 32, 82, 58, 72, 58, 72, 78, 56, 78, 56, 58, 46, 58 };
            var marks = new NativeSim.CampaignMinimapMark[7];
            for (var i = 0; i < 7; i++)
                marks[i] = new NativeSim.CampaignMinimapMark {
                    abiVersion = NativeSim.AbiVersion, structSize = 48, reserved = new uint[3],
                    generation = 1, tick = 66, visible = 1, kind = 3, sourceOrder = (ushort)i,
                    x = xy[i * 2], y = xy[i * 2 + 1], paletteIndex = 123 };
            NativeCampaignSession.ValidateMinimapMarks(marks, 1, 66);
            Assert.Throws<InvalidOperationException>(() =>
                NativeCampaignSession.ValidateMinimapMarks(new[] { marks[0] }, 1, 66));
            Assert.That(CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 66), Is.GreaterThan(30));
            Assert.That(pixels[16 * 320 + 32], Is.EqualTo(123));
            Assert.That(pixels[39 * 320 + 32], Is.EqualTo(123));
            Assert.That(pixels[32 * 320 + 32], Is.Zero, "outline has no invented fill");
            marks[6].tick = 65;
            Assert.Throws<InvalidOperationException>(() =>
                CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 66));
            Assert.Throws<InvalidOperationException>(() =>
                CampaignPresentationSurface.MaterializeMinimapMarks(pixels, new[] { marks[0] }, tables, 1, 66));
            marks[6].tick = 66;
            for (var i = 0; i < 7; i++) marks[i].visible = 0;
            Assert.That(CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 66), Is.Zero);
        }

        [Test]
        public void ObjectiveMarkDrawsSourceOutlineAndHonorsNativePulseAndMargin()
        {
            var pixels = new byte[320 * 200];
            var tables = new byte[83456];
            for (var i = 0; i < 256; i++) tables[0x14300 + i] = 255;
            pixels[32 * 320 + 32] = 17;
            var marks = new[] { new NativeSim.CampaignMinimapMark {
                generation = 1, tick = 4, visible = 1, kind = 2,
                x = 64, y = 64, paletteIndex = 188 } };
            Assert.That(CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 4), Is.EqualTo(8));
            Assert.That(pixels[32 * 320 + 32], Is.EqualTo(17));
            Assert.That(pixels[31 * 320 + 31], Is.EqualTo(188));
            marks[0].visible = 0;
            Assert.That(CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 4), Is.Zero);
            marks[0].visible = 1;
            marks[0].x = 2;
            Assert.That(CampaignPresentationSurface.MaterializeMinimapMarks(pixels, marks, tables, 1, 4), Is.Zero);
        }

        [Test]
        public void OriginalOwnerPlayerSymbolDrawsItsMaskAtNativeBottomAnchor()
        {
            var root = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../generated/level3-day-minimap-owner-cache-v3"));
            var cache = OwnerAssetCache.OpenAndValidateLevel3(root);
            var row = cache.Level3Materials.minimap_symbols.symbols[8];
            Assert.That(row.selector, Is.EqualTo(66));
            var symbol = new CampaignPresentationSurface.MinimapSymbolPixels {
                Width = row.width, Height = row.height,
                Indices = cache.ReadLevel3MaterialOutput(row.indices_path),
                Mask = cache.ReadLevel3MaterialOutput(row.mask_path)
            };
            var frame = new byte[320 * 200];
            var commands = new[] { new NativeSim.CampaignMinimapEntity {
                generation = 1, tick = 2, visible = 1, kind = 2,
                selector = 66, x = 64, y = 64, anchor = 3 } };
            var written = CampaignPresentationSurface.MaterializeMinimapPixels(frame, commands,
                cache.ReadLevel3MaterialOutput("palette/tablesd.u8"), 1, 2,
                new Dictionary<int, CampaignPresentationSurface.MinimapSymbolPixels> { [66] = symbol });
            Assert.That(written, Is.GreaterThan(0));
            for (var y = 0; y < row.height; y++)
            for (var x = 0; x < row.width; x++)
            {
                var source = y * row.width + x;
                Assert.That(frame[(32 - row.height + y) * 320 + 32 - row.width / 2 + x],
                    Is.EqualTo(symbol.Mask[source] == 0 ? 0 : symbol.Indices[source]));
            }
            Assert.That(frame[32 * 320 + 32], Is.Zero, "bottom anchor is below the sprite");
        }

        [Test]
        public void NativeCommandsDrawInOrderAndDoNotInventMissingSprites()
        {
            var pixels = new byte[320 * 200];
            var tables = new byte[83456];
            for (var i = 0; i < 256; i++) tables[0x14300 + i] = 255;
            tables[0x4000 + (17 << 8) + 23] = 91;
            var commands = new[]
            {
                new NativeSim.CampaignMinimapEntity { generation = 1, tick = 2,
                    visible = 1, kind = 1, x = 64, y = 64, size = 1, paletteIndex = 17 },
                new NativeSim.CampaignMinimapEntity { generation = 1, tick = 2,
                    visible = 1, kind = 3, x = 64, y = 64, paletteIndex = 23 },
                new NativeSim.CampaignMinimapEntity { generation = 1, tick = 2,
                    visible = 1, kind = 2, x = 60, y = 60, selector = 66 },
                new NativeSim.CampaignMinimapEntity { generation = 1, tick = 2,
                    visible = 0, kind = 1, x = 64, y = 64, paletteIndex = 55 }
            };
            Assert.That(CampaignPresentationSurface.MaterializeMinimapPixels(
                pixels, commands, tables, 1, 2), Is.EqualTo(2));
            Assert.That(pixels[32 * 320 + 32], Is.EqualTo(91));
            Assert.That(pixels[30 * 320 + 30], Is.Zero);
            Assert.Throws<InvalidOperationException>(() =>
                CampaignPresentationSurface.MaterializeMinimapPixels(pixels, commands, tables, 1, 3));
        }
    }
}
