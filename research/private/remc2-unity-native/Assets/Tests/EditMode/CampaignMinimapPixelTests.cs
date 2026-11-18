using System;
using System.IO;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Runtime.Serialization.Json;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class CampaignMinimapPixelTests
    {
        private sealed class ObjectiveHelpWindow : UnityEditor.EditorWindow
        {
            internal Action RepaintRuntime;
            private void OnGUI() => RepaintRuntime?.Invoke();
        }

        [Test]
        public void ProductionObjectiveHelpReadsLiveDayMarksAcrossBlinkWithoutRepaintAdvancing()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var locale = Environment.GetEnvironmentVariable("MC_CAMPAIGN_MENU_CACHE");
            var replayPath = Environment.GetEnvironmentVariable("MC_OBJECTIVE_HELP_DAY_REPLAY");
            Assert.That(locale, Is.Not.Empty.And.Not.Null);
            Assert.That(replayPath, Is.Not.Empty.And.Not.Null);
            var arguments = new List<string> {
                "-mc-game-runtime", "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "-mc-runtime-menu-cache", locale, "-mc-runtime-replay-load", replayPath
            };
            var resourcesPath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                Path.Combine(generated, "continuity-20261115/installed-full-resources.json");
            using (var input = File.OpenRead(resourcesPath))
            {
                var serializer = new DataContractJsonSerializer(typeof(GameRuntimeTransport.InstalledPackageResource[]));
                var resources = (GameRuntimeTransport.InstalledPackageResource[])serializer.ReadObject(input);
                Assert.That(resources.Length, Is.GreaterThanOrEqualTo(3));
                foreach (var package in resources.Take(3))
                    arguments.AddRange(new[] { "-mc-runtime-package", package.package_dir,
                        "-mc-runtime-owner-cache", package.owner_cache_path,
                        "-mc-runtime-presentation-cache", package.presentation_cache_path });
            }
            var root = new GameObject("Day objective mark help");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var window = ScriptableObject.CreateInstance<ObjectiveHelpWindow>();
            var target = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var shown = false;
            const BindingFlags fields = BindingFlags.Instance | BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, fields).GetValue(runtime);
            var activeBefore = NativeSession.ActiveSessionCount;
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                var native = (NativeCampaignSession)Field("campaignSession");
                var surface = (CampaignPresentationSurface)(Field("gameRuntimeSecretSurface") ?? Field("level3PresentationSurface"));
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                var hint = (MagicCarpetRuntime.ManaHelpHint)Field("gameRuntimeManaHint");
                Assert.That(native.Tick, Is.EqualTo(2025));
                Assert.That(hint.HasObjectiveMarkHelp, Is.True);
                Assert.That(menu.GameplayHelpForRow(81).TextRecord, Is.EqualTo(343));
                var binding = menu.GameplayHelpForRow(82);
                Assert.That((binding.TextRecord, binding.Duration, binding.AgeThreshold, binding.Priority),
                    Is.EqualTo((344, 50, 160, 5)));
                var marks = native.MinimapMarks(native.Tick, native.MinimapState(native.Tick));
                Assert.That(marks.Where(row => row.helpSelector == 82).All(row => row.visible == 0), Is.True);
                Assert.That(surface.ObjectiveHelpMarks.Any(mark => mark.Row == 82 && mark.Subject != 0), Is.True,
                    "ordinary authored rectangles offer help before the next visible blink");
                for (var frame = 0; frame < 80 && hint.VisibleRow != 82; frame++)
                    runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(hint.VisibleRow, Is.EqualTo(82));
                var subjectField = typeof(MagicCarpetRuntime.ManaHelpHint).GetField("objectiveHelpSubject", fields);
                var subject = (ushort)subjectField.GetValue(hint);
                var current = surface.ObjectiveHelpMarks.Single(mark => mark.Row == 82 && mark.Subject == subject);
                Assert.That((hint.AnchorX, hint.AnchorY), Is.EqualTo((current.X, current.Y)));

                foreach (var camera in surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>()
                    .OrderBy(camera => camera.depth))
                {
                    if (camera.targetTexture == null) camera.targetTexture = target;
                    camera.Render();
                }
                window.RepaintRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", fields).Invoke(runtime, null);
                window.minSize = new Vector2(640, 480);
                window.Show(); shown = true; window.position = new Rect(0, 0, 640, 480);
                var tick = native.Tick;
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                var lifetime = typeof(MagicCarpetRuntime.ManaHelpHint).GetField("remaining", fields);
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(49));
                window.SendEvent(new Event { type = EventType.Layout });
                window.SendEvent(new Event { type = EventType.Repaint });
                var presenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                Assert.That(presenter, Is.Not.Null);
                var sourceText = menu.GameplayLanguageText(344);
                Assert.That(sourceText, Is.Not.Empty);
                Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("currentText", fields).GetValue(presenter),
                    Is.EqualTo(sourceText));
                var raster = (ObjectiveTextboxCompositor.Raster)typeof(SourceObjectiveTextboxPresenter)
                    .GetField("raster", fields).GetValue(presenter);
                Assert.That(raster, Is.Not.Null);
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                window.SendEvent(new Event { type = EventType.Repaint });
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(49));
                Assert.That(native.Tick, Is.EqualTo(tick));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(48), "only the next ordinary frame advances help");
            }
            finally
            {
                window.RepaintRuntime = null;
                if (shown) window.Close(); else UnityEngine.Object.DestroyImmediate(window);
                runtime.QualificationDisposeGameRuntimeProduction();
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(root);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
            }
        }

        [Test]
        public void ProductionObjectiveExpiryPromotesMinimapHelpOnlyAfterContextCloses()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var locale = Environment.GetEnvironmentVariable("MC_CAMPAIGN_MENU_CACHE");
            var replayPath = Environment.GetEnvironmentVariable("MC_OBJECTIVE_HELP_NIGHT_REPLAY");
            Assert.That(locale, Is.Not.Empty.And.Not.Null);
            Assert.That(replayPath, Is.Not.Empty.And.Not.Null);
            var arguments = new List<string> {
                "-mc-game-runtime", "-mc-runtime-loading-cache", Path.Combine(generated, "mc2-level1-night-cache-v2"),
                "-mc-runtime-menu-cache", locale, "-mc-runtime-replay-load", replayPath
            };
            var resourcesPath = Environment.GetEnvironmentVariable("MC_CAMPAIGN_SAVE_RESOURCES") ??
                Path.Combine(generated, "continuity-20261115/installed-full-resources.json");
            using (var input = File.OpenRead(resourcesPath))
            {
                var serializer = new DataContractJsonSerializer(typeof(GameRuntimeTransport.InstalledPackageResource[]));
                var resources = (GameRuntimeTransport.InstalledPackageResource[])serializer.ReadObject(input);
                Assert.That(resources.Length, Is.EqualTo(30));
                foreach (var package in resources)
                    arguments.AddRange(new[] { "-mc-runtime-package", package.package_dir,
                        "-mc-runtime-owner-cache", package.owner_cache_path,
                        "-mc-runtime-presentation-cache", package.presentation_cache_path });
            }
            var root = new GameObject("Night objective expiry help");
            root.SetActive(false);
            var runtime = root.AddComponent<MagicCarpetRuntime>();
            var window = ScriptableObject.CreateInstance<ObjectiveHelpWindow>();
            var target = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear);
            var shown = false;
            const BindingFlags fields = BindingFlags.Instance | BindingFlags.NonPublic;
            object Field(string name) => typeof(MagicCarpetRuntime).GetField(name, fields).GetValue(runtime);
            var activeBefore = NativeSession.ActiveSessionCount;
            try
            {
                runtime.QualificationInitializeGameRuntimeProductionFromArguments(arguments.ToArray());
                var native = (NativeCampaignSession)Field("campaignSession");
                var session = (NativeSession)Field("session");
                var surface = (CampaignPresentationSurface)(Field("gameRuntimeSecretSurface") ?? Field("gameRuntimeNightSurface"));
                var menu = (CampaignMenuPresenter)Field("gameRuntimeMenu");
                var hint = (MagicCarpetRuntime.ManaHelpHint)Field("gameRuntimeManaHint");
                Assert.That(native.Tick, Is.EqualTo(169));
                Assert.That(hint.HasObjectiveMarkHelp, Is.True);
                var hintType = typeof(MagicCarpetRuntime.ManaHelpHint);
                var lifetime = hintType.GetField("remaining", fields);
                var rows = ((Array)hintType.GetField("rows", fields).GetValue(hint)).Cast<object>()
                    .ToDictionary(row => ((OwnerCampaignMenuCache.GameplayHelpBinding)
                        row.GetType().GetField("Binding", fields).GetValue(row)).Row);
                int Priority(int row) => (int)rows[row].GetType().GetField("Priority", fields).GetValue(rows[row]);

                // Hold O at the real NightSpire stage checkpoint, then let the
                // client release it and dispatch the subsequent native expiry.
                session.GameRuntimeCampaignStep(0, 0, 0, 0, 0, showObjective: true);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(native.Tick, Is.EqualTo(171));
                for (var frame = 0; frame < 9 && hint.VisibleRow != 79; frame++)
                    runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(hint.VisibleRow, Is.EqualTo(79));
                Assert.That((Priority(2), Priority(79), Priority(81), Priority(82)), Is.EqualTo((2, 6, 5, 5)));
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(49));

                foreach (var camera in surface.FrameCamera.transform.parent.GetComponentsInChildren<Camera>()
                    .OrderBy(camera => camera.depth))
                {
                    if (camera.targetTexture == null) camera.targetTexture = target;
                    camera.Render();
                }
                window.RepaintRuntime = () => typeof(MagicCarpetRuntime).GetMethod("OnGUI", fields).Invoke(runtime, null);
                window.minSize = new Vector2(640, 480);
                window.Show(); shown = true; window.position = new Rect(0, 0, 640, 480);
                var tick = native.Tick;
                var replay = runtime.QualificationSaveGameRuntimeProductionReplay();
                window.SendEvent(new Event { type = EventType.Layout });
                window.SendEvent(new Event { type = EventType.Repaint });
                var presenter = (SourceObjectiveTextboxPresenter)Field("gameRuntimeManaHintPresenter");
                Assert.That(presenter, Is.Not.Null);
                Assert.That(menu.GameplayHelpForRow(79).TextRecord, Is.EqualTo(341));
                var sourceText = menu.GameplayLanguageText(341);
                Assert.That(sourceText, Is.Not.Empty);
                Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("currentText", fields).GetValue(presenter),
                    Is.EqualTo(sourceText));
                Assert.That(typeof(SourceObjectiveTextboxPresenter).GetField("raster", fields).GetValue(presenter),
                    Is.InstanceOf<ObjectiveTextboxCompositor.Raster>());
                runtime.QualificationRefreshGameRuntimeProductionEvents();
                window.SendEvent(new Event { type = EventType.Repaint });
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(49));
                Assert.That((Priority(2), Priority(81)), Is.EqualTo((2, 5)));
                Assert.That(native.Tick, Is.EqualTo(tick));
                CollectionAssert.AreEqual(replay, runtime.QualificationSaveGameRuntimeProductionReplay());

                runtime.QualificationDriveGameRuntimeFrames(48);
                Assert.That(hint.VisibleRow, Is.EqualTo(79));
                Assert.That(lifetime.GetValue(hint), Is.EqualTo(1));
                Assert.That((Priority(2), Priority(81)), Is.EqualTo((2, 5)));
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(lifetime.GetValue(hint), Is.Zero);
                Assert.That((Priority(2), Priority(79), Priority(81), Priority(82)), Is.EqualTo((6, 3, 6, 5)));
                runtime.QualificationDriveGameRuntimeFrames(20);
                Assert.That(hint.VisibleRow, Is.Zero);
                runtime.QualificationDriveGameRuntimeFrames(1);
                Assert.That(hint.VisibleRow, Is.EqualTo(2), "the next source poll gives the promoted minimap row its turn");
            }
            finally
            {
                window.RepaintRuntime = null;
                if (shown) window.Close(); else UnityEngine.Object.DestroyImmediate(window);
                runtime.QualificationDisposeGameRuntimeProduction();
                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
                UnityEngine.Object.DestroyImmediate(root);
                Assert.That(NativeSession.ActiveSessionCount, Is.EqualTo(activeBefore));
            }
        }

        [Test]
        public void IndexedMinimapComposesSourceTerrainAndOrderedWritesBeforeHud()
        {
            // Component controls use the real selected TABLESN. They do not
            // claim an earned campaign or naturally overlapping map entities.
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(
                Path.Combine(generated, "mc-oi62-environment-level1-v2"),
                Path.Combine(generated, "mc-bhq8-level1-collision-v5-night"));
            var tables = cache.ReadOutput(cache.EnvironmentMaterials.tables.path);
            var host = new GameObject("source minimap indexed control");
            host.SetActive(false);
            var overlay = host.AddComponent<CampaignMinimapOverlay>();
            var scene = new Texture2D(320, 200, TextureFormat.R8, false, true) {
                filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
            };
            var readback = new Texture2D(320, 200, TextureFormat.RGBA32, false, true);
            var gpuWorld = new RenderTexture(320, 200, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.Linear) {
                filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp
            };
            Assert.That(gpuWorld.Create(), Is.True);
            var world = new byte[64000]; var terrain = new byte[64000]; var mask = new byte[4096];
            for (var y = 0; y < 200; y++)
            for (var x = 0; x < 320; x++)
            {
                world[y * 320 + x] = unchecked((byte)(x + 13 * y));
                terrain[y * 320 + x] = unchecked((byte)(17 * x + 31 * y));
            }
            for (var y = 0; y < 64; y++)
            {
                var phase = y * 2 < 64 ? 63 - y * 2 : y * 2 - 64;
                var halfSpan = (64 * tables[0x14300 + phase * 4]) >> 8;
                for (var x = 0; x < 64; x++)
                    mask[y * 64 + x] = (byte)(x * 2 >= 64 - halfSpan && x * 2 < 64 + halfSpan ? 1 : 0);
            }
            // Every world index occurs inside the true source circular aperture.
            for (var y = 24; y < 40; y++)
            for (var x = 24; x < 40; x++)
            {
                Assert.That(mask[y * 64 + x], Is.EqualTo(1));
                world[y * 320 + x] = (byte)((y - 24) * 16 + x - 24);
            }
            mask[30 * 64 + 30] = 0; // declared outdoor angle8 transparent hole
            scene.LoadRawTextureData(world); scene.Apply(false, false);
            Color32[] Read(RenderTexture target)
            {
                var previous = RenderTexture.active;
                try { RenderTexture.active = target; readback.ReadPixels(new Rect(0, 0, 320, 200), 0, 0); readback.Apply(); }
                finally { RenderTexture.active = previous; }
                return readback.GetPixels32();
            }
            byte At(Color32[] pixels, int x, int y) => pixels[(199 - y) * 320 + x].r;
            byte Blend(byte prior, byte next) => tables[0x4000 + (prior << 8) + next];
            try
            {
                overlay.BeginIndexedFrame(terrain, tables, mask, false, false);
                var opOffset = 32 * 320 + 32;
                overlay.RecordPixel(opOffset, 77, false);
                for (var operation = 0; operation < 6; operation++)
                    overlay.RecordPixel(opOffset, (byte)(91 + operation), true);
                var labels = new bool[64000]; labels[20 * 320 + 80] = true;
                terrain[20 * 320 + 80] = 44;
                overlay.RecordLabels(terrain, labels);
                var result = Read(overlay.Compose(scene, true, 1, 1));
                for (var y = 24; y < 40; y++)
                for (var x = 24; x < 40; x++)
                {
                    if (x == 32 && y == 32) continue;
                    var expected = mask[y * 64 + x] != 0 ? Blend(world[y * 320 + x], terrain[y * 320 + x]) : world[y * 320 + x];
                    Assert.That(At(result, x, y), Is.EqualTo(expected), $"source terrain/world index at {x},{y}");
                }
                byte ordered = 77;
                for (var operation = 0; operation < 6; operation++) ordered = Blend(ordered, (byte)(91 + operation));
                Assert.That(At(result, 32, 32), Is.EqualTo(ordered), "no three-blend truncation or flattened-world approximation");
                Assert.That(At(result, 80, 20), Is.EqualTo(44), "opaque label outside map aperture");
                Assert.That(At(result, 0, 0), Is.EqualTo(world[0]), "outside aperture retains world");
                overlay.SetPanelsOpaque(true);
                result = Read(overlay.Compose(scene, true, 1, 1));
                Assert.That(At(result, 30, 30), Is.EqualTo(terrain[30 * 320 + 30]), "same-tick opaque mode draws source angle8 cell");
                Assert.That(At(result, 32, 32), Is.EqualTo(ordered), "later opaque/blended marks keep source order");
                overlay.BeginIndexedFrame(terrain, tables, mask, false, true);
                overlay.SetPanelsOpaque(false);
                var cave = Read(overlay.Compose(scene, true, 1, 1));
                overlay.SetPanelsOpaque(true);
                CollectionAssert.AreEqual(cave, Read(overlay.Compose(scene, true, 1, 1)), "Cave minimap stays opaque");
                overlay.BeginIndexedFrame(terrain, tables, null, true, false);
                overlay.SetPanelsOpaque(false);
                var full = Read(overlay.Compose(scene, true, 1, 1));
                Assert.That(At(full, 32, 32), Is.EqualTo(world[32 * 320 + 32]), "full-map scene is already opaque, no circular second layer");
                var previous = RenderTexture.active; var srgb = GL.sRGBWrite;
                try { GL.sRGBWrite = false; Graphics.Blit(overlay.Compose(scene, true, 1, 1), gpuWorld); }
                finally { GL.sRGBWrite = srgb; RenderTexture.active = previous; }
                overlay.BeginIndexedFrame(terrain, tables, mask, false, false);
                result = Read(overlay.Compose(gpuWorld, false, 1, 2));
                for (var y = 24; y < 40; y++)
                for (var x = 24; x < 40; x++)
                {
                    var expected = mask[y * 64 + x] != 0 ? Blend(world[y * 320 + x], terrain[y * 320 + x]) : world[y * 320 + x];
                    Assert.That(At(result, x, y), Is.EqualTo(expected), "GPU scene history has the same source orientation");
                }
                var webIndices = new byte[64000]; var webMask = new byte[64000];
                webIndices[31 * 320 + 32] = 223; webMask[31 * 320 + 32] = 255;
                overlay.SetIndexedWeb(webIndices, webMask, 320, 200); overlay.PresentWeb(null, true);
                result = Read(overlay.Compose(gpuWorld, false, 1, 2));
                Assert.That(At(result, 32, 31), Is.EqualTo(Blend(223, terrain[31 * 320 + 32])), "WEB is below transparent terrain");
                overlay.SetIconsAndMapVisible(false);
                Assert.That(At(Read(overlay.Compose(gpuWorld, false, 1, 2)), 32, 31), Is.EqualTo(223), "hidden minimap preserves independent WEB");
                overlay.BeginIndexedFrame(terrain, tables, null, true, false);
                Assert.That(At(Read(overlay.Compose(gpuWorld, false, 1, 2)), 32, 31), Is.EqualTo(world[31 * 320 + 32]), "opaque full map covers WEB");
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(readback);
                gpuWorld.Release(); UnityEngine.Object.DestroyImmediate(gpuWorld);
                UnityEngine.Object.DestroyImmediate(scene);
                overlay.ReleaseResources(); // inactive component has no OnDestroy lifecycle guarantee
                UnityEngine.Object.DestroyImmediate(host);
            }
        }

        [Serializable]
        private sealed class CacheIdentity { public string cache_content_sha256; }

        private static void AssertNativeSpriteJoin(MagicCarpetRuntime runtime, CampaignPresentationSurface surface)
        {
            var campaign = (NativeCampaignSession)typeof(MagicCarpetRuntime).GetField("campaignSession",
                System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(runtime);
            var snapshots = campaign.Entities();
            var yaw = runtime.GameRuntimeProductionCampaignSnapshot.value.player_pose.yaw;
            var draws = campaign.EntityPresentations(yaw);
            var markers = surface.Root.GetComponentsInChildren<CampaignEntitySurfaceMarker>();
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
            var night = Path.Combine(generated, "mc2-level1-night-cache-v2");
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
                    var marker = Array.Find(surface.Root.GetComponentsInChildren<CampaignEntitySurfaceMarker>(),
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
                    var marker = Array.Find(surface.Root.GetComponentsInChildren<CampaignEntitySurfaceMarker>(),
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
        public void EnvironmentOutputChangedAfterOpenIsRejected()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var source = Path.Combine(generated, "mc-oi62-environment-level1-v2");
            var package = Path.Combine(generated, "mc-bhq8-level1-collision-v5-night");
            var copy = Path.Combine(Path.GetTempPath(), "mc-environment-read-" + Guid.NewGuid().ToString("N"));
            try
            {
                foreach (var directory in Directory.GetDirectories(source, "*", SearchOption.AllDirectories))
                    Directory.CreateDirectory(Path.Combine(copy, Path.GetRelativePath(source, directory)));
                foreach (var file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
                    File.Copy(file, Path.Combine(copy, Path.GetRelativePath(source, file)));
                var cache = OwnerAssetCache.OpenAndValidateEnvironment(copy, package);
                var relative = cache.EnvironmentMaterials.tables.path;
                var bytes = cache.ReadOutput(relative);
                bytes[0] ^= 1;
                File.WriteAllBytes(Path.Combine(copy, relative), bytes);
                Assert.Throws<InvalidDataException>(() => cache.ReadOutput(relative));
                Assert.Throws<InvalidDataException>(() => OwnerAssetCache.OpenAndValidateEnvironment(copy, package));
            }
            finally
            {
                if (Directory.Exists(copy)) Directory.Delete(copy, true);
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
        public void AdmittedFireNightMaterialsReachNativeAndUnitySurface()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var package = Path.Combine(generated, "mc-bhq10-owner-v3-level25-20261029");
            var cachePath = Path.Combine(generated, "dragon-bee-20261105-integration/current-level25-environment");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
            var profile = cache.EnvironmentMaterials.profile;
            Assert.That(profile.terrain_graphics_flags & 2, Is.EqualTo(2));
            Assert.That(profile.palette, Is.EqualTo("PALF-0"));
            Assert.That(profile.blocks, Is.EqualTo("BL32F0-0"));
            Assert.That(profile.dormant_blocks16, Is.EqualTo("BL16F0-0"));
            Assert.That(profile.tables, Is.EqualTo("TABLESN"));
            Assert.That(profile.texture_size, Is.EqualTo(32));
            using (var session = NativeCampaignSession.Open(package,
                NativeSim.CampaignSelection.Create(25, 24, 1), cachePath))
            using (var surface = new CampaignPresentationSurface())
            {
                surface.PresentEnvironment(session, cache);
                Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                Assert.That(surface.CacheContentHash, Is.EqualTo(cache.ContentHash));
                Assert.That(surface.FrameHash, Is.Not.Zero);
                CollectionAssert.AreEqual(cache.ReadOutput(cache.EnvironmentMaterials.palette.rgb8_path),
                    session.DayIndexedFramePalette, "native frame uses the selected PALF palette");
                CollectionAssert.AreEqual(session.DayIndexedFramePalette, surface.FramePaletteRgb8,
                    "Unity consumes the native indexed palette");
                CollectionAssert.AreEqual(session.SceneFrameIndices, surface.WorldIndices.GetRawTextureData<byte>().ToArray(),
                    "Unity receives the native indexed scene pixels");
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
        public void RealCave12ResetMaterialsReachNativeAndUnityConsumers()
        {
            var generated = Path.GetFullPath(Path.Combine(Application.dataPath, "../../generated"));
            var package = Path.Combine(generated, "mc-bhq6-ordinary-20261025-zclu5fjj/level12");
            var cachePath = Path.Combine(generated, "mc-oi62-environment-level12-v1");
            var cache = OwnerAssetCache.OpenAndValidateEnvironment(cachePath, package);
            Assert.That(cache.EnvironmentMaterials.profile.palette, Is.EqualTo("PALC-0"));
            Assert.That(cache.EnvironmentMaterials.sky, Is.Null);
            using (var session = NativeCampaignSession.Open(package,
                NativeSim.CampaignSelection.Create(12, 11, 2), cachePath))
            using (var surface = new CampaignPresentationSurface())
            {
                Assert.That(session.Tick, Is.Zero);
                var players = session.Players();
                var player = Array.Find(players, row => row.slot == 0 && row.isAi == 0 && row.active == 1);
                Assert.That(player.active, Is.EqualTo(1));
                var entities = session.Entities();
                var camera = Array.Find(entities, row => row.entityIndex == player.entityIndex);
                Assert.That(camera.active, Is.EqualTo(1));
                Assert.That(camera.x, Is.EqualTo(29056));
                Assert.That(camera.y, Is.EqualTo(14208));
                Assert.That(camera.z, Is.EqualTo(1168));
                surface.PresentEnvironment(session, cache, camera.yaw);
                Assert.That(surface.WorldReady && surface.LiveMinimapReady, Is.True);
                Assert.That(surface.CacheContentHash, Is.EqualTo(cache.ContentHash));
                Assert.That(surface.Generation, Is.EqualTo(session.Generation));
                Assert.That(surface.Tick, Is.Zero);
                Assert.That(surface.FrameHash, Is.Not.Zero);
                Assert.That(session.CampaignFrameInfo.flags & NativeSim.TerrainFrameSkySeeded, Is.Zero);
                Assert.That(session.CampaignFrameInfo.flags & NativeSim.TerrainFrameNonTerrainOmitted, Is.Not.Zero);
                var first = surface.FrameHash;
                surface.PresentEnvironment(session, cache, camera.yaw);
                Assert.That(surface.FrameHash, Is.EqualTo(first));
                Assert.That(session.Tick, Is.Zero, "material presentation does not dispatch the unsupported first gameplay turn");
                var wrong = OwnerAssetCache.OpenAndValidateEnvironment(
                    Path.Combine(generated, "mc-oi62-environment-level2-v2"),
                    Path.Combine(generated, "level2-day-block32-campaign-v2"));
                Assert.Throws<InvalidOperationException>(() => surface.PresentEnvironment(session, wrong));
                Assert.That(surface.FrameHash, Is.EqualTo(first));
                Assert.That(surface.WorldReady, Is.True);
                Assert.That(session.WorldInfo().profile, Is.EqualTo(1),
                    "MC_PROFILE_CAVE must describe the actual Cave reset, after the real frame/surface checks");
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
