#if UNITY_EDITOR
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace MagicCarpet.PresentationLab.Editor
{
    public static class CreatePresentationLabAssets
    {
        private const string RendererPath = "Assets/Settings/PresentationLabRenderer.asset";
        private const string PipelinePath = "Assets/Settings/PresentationLabURP.asset";

        [MenuItem("Magic Carpet/Presentation Lab/Create URP Assets")]
        public static void CreateRenderPipelineAssets()
        {
            EnsureFolder("Assets/Settings");

            UniversalRendererData renderer = AssetDatabase.LoadAssetAtPath<UniversalRendererData>(RendererPath);
            if (renderer == null)
            {
                renderer = ScriptableObject.CreateInstance<UniversalRendererData>();
                renderer.name = "PresentationLabRenderer";
                AssetDatabase.CreateAsset(renderer, RendererPath);
            }

            UniversalRenderPipelineAsset pipeline = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(PipelinePath);
            if (pipeline == null)
            {
                pipeline = UniversalRenderPipelineAsset.Create(renderer);
                pipeline.name = "PresentationLabURP";
                AssetDatabase.CreateAsset(pipeline, PipelinePath);
            }

            GraphicsSettings.defaultRenderPipeline = pipeline;
            EditorUtility.SetDirty(renderer);
            EditorUtility.SetDirty(pipeline);
            AssetDatabase.SaveAssets();
            AssetDatabase.Refresh();
            Debug.Log("Presentation Lab URP assets created and assigned: " + PipelinePath);
        }

        [MenuItem("Magic Carpet/Presentation Lab/Validate Boot Scene")]
        public static void ValidateBootScene()
        {
            SceneAsset sceneAsset = AssetDatabase.LoadAssetAtPath<SceneAsset>("Assets/Scenes/Boot.unity");
            if (sceneAsset == null)
            {
                throw new System.InvalidOperationException("Boot scene asset is missing");
            }

            var scene = EditorSceneManager.OpenScene("Assets/Scenes/Boot.unity", OpenSceneMode.Single);
            if (!scene.IsValid() || scene.rootCount < 2)
            {
                throw new System.InvalidOperationException("Boot scene did not load with the router and camera roots");
            }

            Debug.Log("Presentation Lab Boot scene validated: " + scene.path);
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Build Standalone Linux Player")]
        public static void BuildStandaloneLinuxPlayer()
        {
            string outputPath = CommandLineValue("-mc-build-output");
            if (string.IsNullOrWhiteSpace(outputPath))
            {
                throw new InvalidOperationException(
                    "standalone presentation player build requires -mc-build-output");
            }

            string absoluteOutputPath = Path.GetFullPath(outputPath);
            if (File.Exists(absoluteOutputPath) || Directory.Exists(absoluteOutputPath))
            {
                throw new InvalidOperationException(
                    "refusing to overwrite an existing standalone player output: " + absoluteOutputPath);
            }

            List<string> scenePaths = new List<string>();
            foreach (EditorBuildSettingsScene scene in EditorBuildSettings.scenes)
            {
                if (scene.enabled && !string.IsNullOrWhiteSpace(scene.path))
                {
                    scenePaths.Add(scene.path);
                }
            }

            if (scenePaths.Count == 0)
            {
                throw new InvalidOperationException(
                    "standalone presentation player build requires at least one enabled scene");
            }

            string outputDirectory = Path.GetDirectoryName(absoluteOutputPath);
            if (!string.IsNullOrEmpty(outputDirectory))
            {
                Directory.CreateDirectory(outputDirectory);
            }

            BuildReport report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = scenePaths.ToArray(),
                locationPathName = absoluteOutputPath,
                target = BuildTarget.StandaloneLinux64,
                options = BuildOptions.StrictMode
            });
            if (report == null || report.summary.result != BuildResult.Succeeded)
            {
                string result = report == null ? "no build report" : report.summary.result.ToString();
                int errors = report == null ? -1 : report.summary.totalErrors;
                throw new InvalidOperationException(
                    "standalone presentation player build failed: result=" + result +
                    " errors=" + errors);
            }

            Debug.Log(
                "Standalone presentation player build PASS: target=StandaloneLinux64" +
                " scenes=" + scenePaths.Count +
                " output=" + absoluteOutputPath +
                " size_bytes=" + new FileInfo(absoluteOutputPath).Length);
            if (Application.isBatchMode)
            {
                EditorApplication.Exit(0);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Smoke")]
        public static void RunOwnerClassicHdSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SMOKE FAIL: router did not open the owner-bound Classic HD route");
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                if (!router.ClassicHdRenderer.CaptureFrame(0f, 0f, out reason) ||
                    router.ClassicHdRenderer.UploadCount == 0 ||
                    router.ClassicHdRenderer.LastNativeIndexHash == 0 ||
                    router.ClassicHdRenderer.LastNativeProvenanceHash == 0)
                {
                    Debug.LogError("OWNER CLASSIC HD SMOKE FAIL: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD SMOKE PASS: receipt=" +
                    router.ClassicHdRenderer.LastNativeReceiptHash.ToString("x16") +
                    " index=" + router.ClassicHdRenderer.LastNativeIndexHash.ToString("x16") +
                    " provenance=" + router.ClassicHdRenderer.LastNativeProvenanceHash.ToString("x16") +
                    " uploads=" + router.ClassicHdRenderer.UploadCount);
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Sequence Smoke")]
        public static void RunOwnerClassicHdSequenceSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD sequence smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Sequence Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int FramesPerPass = 32;
            ulong[] firstIndexHashes = new ulong[FramesPerPass];
            ulong[] firstProvenanceHashes = new ulong[FramesPerPass];
            ulong firstGeneration;
            ulong firstTick;
            ulong secondGeneration;
            ulong secondTick;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SEQUENCE SMOKE FAIL: router did not open the owner-bound Classic HD route");
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                for (int frame = 0; frame < FramesPerPass; frame++)
                {
                    Vector2 input = SequenceInput(frame);
                    if (!router.ClassicHdRenderer.CaptureFrame(input.x, input.y, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SEQUENCE SMOKE FAIL: first pass frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    firstIndexHashes[frame] = router.ClassicHdRenderer.LastNativeIndexHash;
                    firstProvenanceHashes[frame] = router.ClassicHdRenderer.LastNativeProvenanceHash;
                    if (firstIndexHashes[frame] == 0 || firstProvenanceHashes[frame] == 0)
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD SEQUENCE SMOKE FAIL: first pass frame " + frame +
                            " returned an empty native hash");
                        EditorApplication.Exit(1);
                        return;
                    }
                }

                firstGeneration = router.ClassicHdRenderer.SourceGeneration;
                firstTick = router.ClassicHdRenderer.SourceTick;
                ulong firstIndexSequenceHash = SequenceHash(firstIndexHashes);
                ulong firstProvenanceSequenceHash = SequenceHash(firstProvenanceHashes);

                if (!router.ClassicHdRenderer.ResetOwnerSession(out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD SEQUENCE SMOKE FAIL: reset: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                ulong[] secondIndexHashes = new ulong[FramesPerPass];
                ulong[] secondProvenanceHashes = new ulong[FramesPerPass];
                for (int frame = 0; frame < FramesPerPass; frame++)
                {
                    Vector2 input = SequenceInput(frame);
                    if (!router.ClassicHdRenderer.CaptureFrame(input.x, input.y, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SEQUENCE SMOKE FAIL: reset pass frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    secondIndexHashes[frame] = router.ClassicHdRenderer.LastNativeIndexHash;
                    secondProvenanceHashes[frame] = router.ClassicHdRenderer.LastNativeProvenanceHash;
                    if (secondIndexHashes[frame] != firstIndexHashes[frame] ||
                        secondProvenanceHashes[frame] != firstProvenanceHashes[frame])
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD SEQUENCE SMOKE FAIL: reset replay diverged at frame " + frame +
                            " first=" + firstIndexHashes[frame].ToString("x16") +
                            "/" + firstProvenanceHashes[frame].ToString("x16") +
                            " second=" + secondIndexHashes[frame].ToString("x16") +
                            "/" + secondProvenanceHashes[frame].ToString("x16"));
                        EditorApplication.Exit(1);
                        return;
                    }
                }

                secondGeneration = router.ClassicHdRenderer.SourceGeneration;
                secondTick = router.ClassicHdRenderer.SourceTick;
                if (router.ClassicHdRenderer.UploadCount != FramesPerPass * 2 ||
                    router.ClassicHdRenderer.AllocationEpoch != 1)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SEQUENCE SMOKE FAIL: uploads=" + router.ClassicHdRenderer.UploadCount +
                        " allocation_epoch=" + router.ClassicHdRenderer.AllocationEpoch);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD SEQUENCE SMOKE PASS: profile=" + router.ClassicHdRenderer.Settings.profile +
                    " frames_per_pass=" + FramesPerPass +
                    " uploads=" + router.ClassicHdRenderer.UploadCount +
                    " allocation_epoch=" + router.ClassicHdRenderer.AllocationEpoch +
                    " first_generation=" + firstGeneration +
                    " first_tick=" + firstTick +
                    " second_generation=" + secondGeneration +
                    " second_tick=" + secondTick +
                    " index_sequence=" + firstIndexSequenceHash.ToString("x16") +
                    " provenance_sequence=" + firstProvenanceSequenceHash.ToString("x16") +
                    " first_index=" + firstIndexHashes[0].ToString("x16") +
                    " last_index=" + firstIndexHashes[FramesPerPass - 1].ToString("x16"));
                EditorApplication.Exit(0);
            }
            finally
            {
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Scenario Smoke")]
        public static void RunOwnerClassicHdScenarioSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD scenario smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Scenario Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int IdleFrames = 8;
            const int TownFrames = 12;
            const int CombatFrames = 12;
            const int FlightFrames = 16;
            var indexRouteHashes = new List<ulong>();
            var provenanceRouteHashes = new List<ulong>();
            int combatProjectiles = 0;
            int combatImpacts = 0;
            short maxFlightSpeed = short.MinValue;
            bool deathObserved = false;
            ulong generationBeforeRestart = 0;
            ulong generationAfterRestart = 0;
            ulong restartTick = ulong.MaxValue;
            long townCameraDistance = 0;
            long flightCameraDistance = 0;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                ClassicHdPresentation classic = router.ClassicHdRenderer;
                string requestedAssetTier = CommandLineValue("-mc-classic-asset-tier");
                ClassicHdAssetTierResolution assetTierResolution = router.ClassicHdAssetTierResolution;
                if (assetTierResolution == null ||
                    (string.Equals(requestedAssetTier, "optional-high-resolution", StringComparison.OrdinalIgnoreCase) &&
                     !assetTierResolution.FallbackApplied))
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: asset-tier resolution was not fail-closed" +
                        " requested=" + requestedAssetTier +
                        " effective=" + (assetTierResolution == null ? "missing" : assetTierResolution.EffectiveToken));
                    EditorApplication.Exit(1);
                    return;
                }
                ushort townStartX = classic.CameraX;
                ushort townStartY = classic.CameraY;
                ushort flightStartX = townStartX;
                ushort flightStartY = townStartY;

                for (int frame = 0; frame < IdleFrames; frame++)
                {
                    if (!classic.CaptureFrame(0f, 0f, false, -1, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: idle frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    if (classic.LastNativeIndexHash == 0 || classic.LastNativeProvenanceHash == 0)
                    {
                        Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: idle frame has an empty indexed receipt");
                        EditorApplication.Exit(1);
                        return;
                    }

                    indexRouteHashes.Add(classic.LastNativeIndexHash);
                    provenanceRouteHashes.Add(classic.LastNativeProvenanceHash);
                }

                townStartX = classic.CameraX;
                townStartY = classic.CameraY;
                for (int frame = 0; frame < TownFrames; frame++)
                {
                    Vector2 input = ScenarioFlightInput("town", frame);
                    if (!classic.CaptureFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: town frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    indexRouteHashes.Add(classic.LastNativeIndexHash);
                    provenanceRouteHashes.Add(classic.LastNativeProvenanceHash);
                }

                townCameraDistance = CameraDistance(classic.CameraX, classic.CameraY, townStartX, townStartY);
                if (townCameraDistance <= 0)
                {
                    Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: town route did not move the authoritative camera");
                    EditorApplication.Exit(1);
                    return;
                }

                for (int frame = 0; frame < CombatFrames; frame++)
                {
                    bool cast = frame == 4;
                    int spellSelection = frame == 3 ? 0 : -1;
                    if (!classic.CaptureFrame(0f, 0f, cast, spellSelection, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: combat frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    combatProjectiles = Math.Max(combatProjectiles, classic.MissionProjectileCount);
                    combatImpacts = Math.Max(combatImpacts, classic.MissionImpactCount);
                    indexRouteHashes.Add(classic.LastNativeIndexHash);
                    provenanceRouteHashes.Add(classic.LastNativeProvenanceHash);
                }

                if (combatProjectiles <= 0 || combatImpacts <= 0)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: combat route did not publish projectile/impact rows" +
                        " projectiles=" + combatProjectiles +
                        " impacts=" + combatImpacts);
                    EditorApplication.Exit(1);
                    return;
                }

                flightStartX = classic.CameraX;
                flightStartY = classic.CameraY;
                for (int frame = 0; frame < FlightFrames; frame++)
                {
                    Vector2 input = ScenarioFlightInput("flight", frame);
                    if (!classic.CaptureFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: flight frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    maxFlightSpeed = Math.Max(maxFlightSpeed, classic.ActualSpeed);
                    indexRouteHashes.Add(classic.LastNativeIndexHash);
                    provenanceRouteHashes.Add(classic.LastNativeProvenanceHash);
                }

                flightCameraDistance = CameraDistance(classic.CameraX, classic.CameraY, flightStartX, flightStartY);
                if (maxFlightSpeed <= 0 || flightCameraDistance <= 0)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: high-speed flight route did not move the authoritative camera" +
                        " speed=" + maxFlightSpeed +
                        " distance=" + flightCameraDistance);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!classic.InjectPlayerDamageForQualification(100000, 10, out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: death setup: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!classic.CaptureFrame(0f, 0f, false, -1, out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: death frame: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (classic.LastNativeIndexHash == 0 || classic.LastNativeProvenanceHash == 0)
                {
                    Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: death frame has an empty indexed receipt");
                    EditorApplication.Exit(1);
                    return;
                }

                indexRouteHashes.Add(classic.LastNativeIndexHash);
                provenanceRouteHashes.Add(classic.LastNativeProvenanceHash);
                deathObserved = classic.PlayerDead || classic.PlayerLife <= 0;
                generationBeforeRestart = classic.SourceGeneration;
                if (!deathObserved)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: lethal owner qualification did not reach player-dead state" +
                        " life=" + classic.PlayerLife +
                        " dead=" + classic.PlayerDead);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!classic.ResetOwnerSession(out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD SCENARIO SMOKE FAIL: restart reset: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                generationAfterRestart = classic.SourceGeneration;
                restartTick = classic.SourceTick;
                if (generationAfterRestart <= generationBeforeRestart || restartTick != 0)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD SCENARIO SMOKE FAIL: restart did not produce a fresh tick-0 generation" +
                        " before=" + generationBeforeRestart +
                        " after=" + generationAfterRestart +
                        " tick=" + restartTick);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD SCENARIO SMOKE PASS: profile=" + classic.Settings.profile +
                    " idle_frames=" + IdleFrames +
                    " town_frames=" + TownFrames +
                    " combat_frames=" + CombatFrames +
                    " flight_frames=" + FlightFrames +
                    " frame_count=" + indexRouteHashes.Count +
                    " uploads=" + classic.UploadCount +
                    " town_camera_distance=" + townCameraDistance +
                    " combat_projectiles=" + combatProjectiles +
                    " combat_impacts=" + combatImpacts +
                    " max_flight_speed=" + maxFlightSpeed +
                    " flight_camera_distance=" + flightCameraDistance +
                    " death_observed=" + (deathObserved ? 1 : 0) +
                    " restart_generation=" + generationAfterRestart +
                    " restart_tick=" + restartTick +
                    " requested_asset_tier=" + (string.IsNullOrWhiteSpace(requestedAssetTier) ? "original-indexed" : requestedAssetTier) +
                    " resolved_asset_tier=" + assetTierResolution.EffectiveToken +
                    " asset_tier_fallback_applied=" + (assetTierResolution.FallbackApplied ? 1 : 0) +
                    " asset_tier_optional_pack_present=" + (assetTierResolution.OptionalPackPresent ? 1 : 0) +
                    " asset_tier_fallback_reason=" + (string.IsNullOrWhiteSpace(assetTierResolution.Reason) ? "none" : assetTierResolution.Reason.Replace(' ', '_')) +
                    " index_route_hash=" + SequenceHash(indexRouteHashes).ToString("x16") +
                    " provenance_route_hash=" + SequenceHash(provenanceRouteHashes).ToString("x16") +
                    " last_receipt_hash=" + classic.LastNativeReceiptHash.ToString("x16") +
                    " owner_inputs_external=true");
                UnityEngine.Object.DestroyImmediate(root);
                root = null;
                EditorApplication.Exit(0);
            }
            finally
            {
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Focus Pause Probe")]
        public static void RunOwnerClassicHdFocusPauseProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD focus/pause probe requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Focus Pause Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError("OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                string reason;
                if (!classic.CaptureFrame(0f, 0f, out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: baseline frame: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                ulong baselineGeneration = classic.AuthoritativeGeneration;
                ulong baselineTick = classic.AuthoritativeTick;
                ulong baselinePresentationTick = classic.SourceTick;
                int baselineUploads = classic.UploadCount;
                if (!router.ApplyApplicationLifecycleForQualification(false, false, out reason) ||
                    !router.IsApplicationSuspended ||
                    !classic.IsApplicationSuspended)
                {
                    Debug.LogError("OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: focus loss did not suspend: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (classic.CaptureFrame(0f, 0f, out reason) ||
                    !classic.IsApplicationSuspended ||
                    classic.AuthoritativeGeneration != baselineGeneration ||
                    classic.AuthoritativeTick != baselineTick ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: paused capture advanced authoritative state" +
                        " baseline_generation=" + baselineGeneration +
                        " baseline_tick=" + baselineTick +
                        " baseline_presentation_tick=" + baselinePresentationTick +
                        " router_suspended=" + router.IsApplicationSuspended +
                        " renderer_suspended=" + classic.IsApplicationSuspended +
                        " generation=" + classic.AuthoritativeGeneration +
                        " tick=" + classic.AuthoritativeTick +
                        " presentation_tick=" + classic.SourceTick +
                        " uploads=" + classic.UploadCount +
                        " reason=" + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!router.ApplyApplicationLifecycleForQualification(true, false, out reason) ||
                    router.IsApplicationSuspended ||
                    classic.IsApplicationSuspended ||
                    !classic.CaptureFrame(0f, 0f, out reason) ||
                    classic.AuthoritativeTick <= baselineTick)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: focus restore did not resume Rust" +
                        " tick=" + classic.AuthoritativeTick +
                        " presentation_tick=" + classic.SourceTick +
                        " reason=" + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!router.ApplyApplicationLifecycleForQualification(true, true, out reason) ||
                    !router.IsApplicationSuspended ||
                    !classic.IsApplicationSuspended)
                {
                    Debug.LogError("OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: application pause did not suspend: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!router.ApplyApplicationLifecycleForQualification(true, false, out reason) ||
                    router.IsApplicationSuspended ||
                    classic.IsApplicationSuspended)
                {
                    Debug.LogError("OWNER CLASSIC HD FOCUS PAUSE PROBE FAIL: application resume did not restore: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD FOCUS PAUSE PROBE PASS: profile=" + classic.Settings.profile +
                    " baseline_generation=" + baselineGeneration +
                    " baseline_tick=" + baselineTick +
                    " baseline_presentation_tick=" + baselinePresentationTick +
                    " resumed_tick=" + classic.SourceTick +
                    " resumed_authoritative_tick=" + classic.AuthoritativeTick +
                    " baseline_uploads=" + baselineUploads +
                    " uploads=" + classic.UploadCount +
                    " lifecycle_transitions=" + classic.ApplicationLifecycleTransitions +
                    " output_restore_requests=" + router.OutputRestoreRequests +
                    " native_pause_contract=pass" +
                    " fullscreen_alt_tab_status=open-os-window-capture-required" +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Exit Sidecar Probe")]
        public static void RunOwnerClassicHdExitSidecarProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD exit sidecar probe requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Exit Sidecar Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError("OWNER CLASSIC HD EXIT SIDECAR PROBE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                string reason;
                if (!classic.CaptureFrame(0f, 0f, out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD EXIT SIDECAR PROBE FAIL: initial frame: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                bool initialSidecarClear = !classic.ExitSequenceActive &&
                                            !classic.ExitSequenceComplete &&
                                            classic.ExitResultCode == 0 &&
                                            !classic.ExitDisplayBlank &&
                                            classic.ExitPlayerAction == 0 &&
                                            classic.ExitActionPhase == 0;
                ulong initialGeneration = classic.SourceGeneration;
                ulong initialTick = classic.SourceTick;
                if (!initialSidecarClear)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD EXIT SIDECAR PROBE FAIL: reset sidecar was not clear" +
                        " active=" + (classic.ExitSequenceActive ? 1 : 0) +
                        " complete=" + (classic.ExitSequenceComplete ? 1 : 0) +
                        " result=" + classic.ExitResultCode);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!classic.ResetOwnerSession(out reason) ||
                    !classic.CaptureFrame(0f, 0f, out reason))
                {
                    Debug.LogError("OWNER CLASSIC HD EXIT SIDECAR PROBE FAIL: reset replay: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                bool resetSidecarClear = !classic.ExitSequenceActive &&
                                          !classic.ExitSequenceComplete &&
                                          classic.ExitResultCode == 0 &&
                                          !classic.ExitDisplayBlank &&
                                          classic.ExitPlayerAction == 0 &&
                                          classic.ExitActionPhase == 0;
                if (!resetSidecarClear ||
                    classic.AuthoritativeGeneration <= initialGeneration ||
                    classic.AuthoritativeTick != 1)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD EXIT SIDECAR PROBE FAIL: reset sidecar replay diverged" +
                        " generation=" + classic.AuthoritativeGeneration +
                        " tick=" + classic.AuthoritativeTick +
                        " presentation_tick=" + classic.SourceTick);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD EXIT SIDECAR PROBE PASS: profile=" + classic.Settings.profile +
                    " initial_generation=" + initialGeneration +
                    " initial_tick=" + initialTick +
                    " reset_generation=" + classic.SourceGeneration +
                    " reset_tick=" + classic.SourceTick +
                    " reset_authoritative_generation=" + classic.AuthoritativeGeneration +
                    " reset_authoritative_tick=" + classic.AuthoritativeTick +
                    " tasks_completed=" + classic.TasksCompleted +
                    " exit_active=" + (classic.ExitSequenceActive ? 1 : 0) +
                    " exit_complete=" + (classic.ExitSequenceComplete ? 1 : 0) +
                    " exit_result=" + classic.ExitResultCode +
                    " authoritative_completion_status=open-full-authored-route-not-driven" +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Effects Matrix Probe")]
        public static void RunOwnerClassicHdEffectsMatrixProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD effects matrix probe requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            const string Profile = "hd-640x480";
            string[] blurModes = { "off", "spatial-2x2", "tablesn" };
            var rootObjects = new List<GameObject>();
            var successfulBlurModes = new List<string>();
            string rejectionReasonReflection = string.Empty;
            string rejectionReasonLighting = string.Empty;

            try
            {
                foreach (string blurMode in blurModes)
                {
                    GameObject root = new GameObject("Owner Classic HD Effects Matrix " + blurMode);
                    rootObjects.Add(root);
                    ClassicHdPresentation presentation = root.AddComponent<ClassicHdPresentation>();
                    ClassicHdPresentationSettings settings = ClassicHdPresentationSettings.Default();
                    settings.profile = Profile;
                    settings.assetTier = ClassicHdAssetTierResolver.OriginalIndexedToken;
                    settings.blur = blurMode;
                    settings.reflections = true;
                    settings.dynamicLighting = true;
                    string reason;
                    if (!presentation.ConfigureOwnerSession(
                            settings,
                            packagePath,
                            closurePath,
                            assetCachePath,
                            out reason) ||
                        !presentation.CaptureFrame(0f, 0f, out reason) ||
                        presentation.UploadCount != 1 ||
                        presentation.LastNativeIndexHash == 0 ||
                        presentation.LastNativeProvenanceHash == 0)
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD EFFECTS MATRIX PROBE FAIL: blur=" + blurMode +
                            " reason=" + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    successfulBlurModes.Add(blurMode);
                    Debug.Log(
                        "OWNER CLASSIC HD EFFECTS MATRIX CASE PASS: profile=" + Profile +
                        " blur=" + blurMode +
                        " reflections=1 dynamic_lighting=1" +
                        " uploads=" + presentation.UploadCount +
                        " allocation_epoch=" + presentation.AllocationEpoch +
                        " effective_postprocess_mode=" + presentation.EffectivePostprocessMode +
                        " index_hash=" + presentation.LastNativeIndexHash.ToString("x16") +
                        " provenance_hash=" + presentation.LastNativeProvenanceHash.ToString("x16"));

                    UnityEngine.Object.DestroyImmediate(root);
                    rootObjects.Remove(root);
                }

                GameObject reflectionRoot = new GameObject("Owner Classic HD Effects Matrix Reflection Rejection");
                rootObjects.Add(reflectionRoot);
                ClassicHdPresentation reflectionPresentation = reflectionRoot.AddComponent<ClassicHdPresentation>();
                ClassicHdPresentationSettings reflectionSettings = ClassicHdPresentationSettings.Default();
                reflectionSettings.profile = Profile;
                reflectionSettings.reflections = false;
                string reflectionReason;
                bool reflectionConfigured = reflectionPresentation.ConfigureOwnerSession(
                    reflectionSettings,
                    packagePath,
                    closurePath,
                    assetCachePath,
                    out reflectionReason);
                if (reflectionConfigured)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD EFFECTS MATRIX PROBE FAIL: reflections-off owner route was accepted");
                    EditorApplication.Exit(1);
                    return;
                }
                rejectionReasonReflection = reflectionReason;

                GameObject lightingRoot = new GameObject("Owner Classic HD Effects Matrix Lighting Rejection");
                rootObjects.Add(lightingRoot);
                ClassicHdPresentation lightingPresentation = lightingRoot.AddComponent<ClassicHdPresentation>();
                ClassicHdPresentationSettings lightingSettings = ClassicHdPresentationSettings.Default();
                lightingSettings.profile = Profile;
                lightingSettings.dynamicLighting = false;
                string lightingReason;
                bool lightingConfigured = lightingPresentation.ConfigureOwnerSession(
                    lightingSettings,
                    packagePath,
                    closurePath,
                    assetCachePath,
                    out lightingReason);
                if (lightingConfigured)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD EFFECTS MATRIX PROBE FAIL: dynamic-lighting-off owner route was accepted");
                    EditorApplication.Exit(1);
                    return;
                }
                rejectionReasonLighting = lightingReason;

                if (successfulBlurModes.Count != blurModes.Length ||
                    string.IsNullOrWhiteSpace(rejectionReasonReflection) ||
                    string.IsNullOrWhiteSpace(rejectionReasonLighting))
                {
                    Debug.LogError("OWNER CLASSIC HD EFFECTS MATRIX PROBE FAIL: settings matrix was incomplete");
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER CLASSIC HD EFFECTS MATRIX PROBE PASS: profile=" + Profile +
                    " blur_modes=" + string.Join(",", successfulBlurModes.ToArray()) +
                    " reflections_on_status=pass dynamic_lighting_on_status=pass" +
                    " reflections_off_status=rejected" +
                    " dynamic_lighting_off_status=rejected" +
                    " reflections_off_reason=" + rejectionReasonReflection.Replace(' ', '_') +
                    " dynamic_lighting_off_reason=" + rejectionReasonLighting.Replace(' ', '_') +
                    " visual_capture_status=open-corpus-capture-required" +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            finally
            {
                for (int index = rootObjects.Count - 1; index >= 0; index--)
                {
                    if (rootObjects[index] != null)
                    {
                        UnityEngine.Object.DestroyImmediate(rootObjects[index]);
                    }
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Visual Capture Probe")]
        public static void RunOwnerClassicHdVisualCaptureProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            string captureDirectory = CommandLineValue("-mc-capture-dir");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath) ||
                string.IsNullOrWhiteSpace(captureDirectory))
            {
                throw new InvalidOperationException(
                    "owner Classic HD visual capture probe requires -mc-package, -mc-closure, -mc-asset-cache, and -mc-capture-dir");
            }

            captureDirectory = Path.GetFullPath(captureDirectory);
            Directory.CreateDirectory(captureDirectory);

            var root = new GameObject("Owner Classic HD Visual Capture Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            string[] routeIds =
            {
                "town-approach",
                "town-overflight",
                "fireball-fight",
                "mana-castle",
                "reflective-water",
                "fog-depth",
                "high-speed-flight"
            };
            string[] captureNames =
            {
                "l1-night-town-approach",
                "l1-night-town-overflight",
                "l1-night-fireball-fight",
                "l1-night-mana-castle",
                "l1-night-reflective-water",
                "l1-night-fog-depth",
                "l1-night-high-speed-flight"
            };
            int[] frameCounts = { 8, 12, 12, 8, 12, 12, 16 };
            var entries = new List<ClassicHdVisualCaptureEntry>();

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD VISUAL CAPTURE PROBE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                string reason = string.Empty;
                for (int routeIndex = 0; routeIndex < routeIds.Length; routeIndex++)
                {
                    if (routeIndex > 0 && !classic.ResetOwnerSession(out reason))
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD VISUAL CAPTURE PROBE FAIL: reset " +
                            routeIds[routeIndex] + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    bool castObserved = false;
                    for (int frame = 0; frame < frameCounts[routeIndex]; frame++)
                    {
                        Vector2 input = VisualCaptureInput(routeIds[routeIndex], frame);
                        bool cast = string.Equals(routeIds[routeIndex], "fireball-fight", StringComparison.Ordinal) &&
                                    frame == 4;
                        int spellSelection = cast ? 0 : -1;
                        if (!classic.CaptureFrame(input.x, input.y, cast, spellSelection, out reason))
                        {
                            Debug.LogError(
                                "OWNER CLASSIC HD VISUAL CAPTURE PROBE FAIL: route=" +
                                routeIds[routeIndex] + " frame=" + frame + ": " + reason);
                            EditorApplication.Exit(1);
                            return;
                        }

                        castObserved |= cast;
                    }

                    byte[] pngBytes;
                    ulong resolvedHash;
                    if (!classic.CaptureResolvedFramePng(out pngBytes, out resolvedHash, out reason))
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD VISUAL CAPTURE PROBE FAIL: resolved capture route=" +
                            routeIds[routeIndex] + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    string fileName = captureNames[routeIndex] + ".png";
                    string outputPath = Path.Combine(captureDirectory, fileName);
                    string receiptPath = Path.Combine(
                        captureDirectory,
                        "classic-hd-owner-visual-capture-receipt.json");
                    if (File.Exists(outputPath) || (routeIndex == 0 && File.Exists(receiptPath)))
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD VISUAL CAPTURE PROBE FAIL: refusing to overwrite existing capture in " +
                            captureDirectory);
                        EditorApplication.Exit(1);
                        return;
                    }

                    File.WriteAllBytes(outputPath, pngBytes);
                    entries.Add(new ClassicHdVisualCaptureEntry
                    {
                        route_id = routeIds[routeIndex],
                        capture_name = captureNames[routeIndex],
                        file_name = fileName,
                        file_sha256 = Sha256Hex(pngBytes),
                        resolved_png_hash = resolvedHash.ToString("x16"),
                        frame_count = frameCounts[routeIndex],
                        cast_observed = castObserved,
                        width = classic.IndexTexture.width,
                        height = classic.IndexTexture.height,
                        upload_count = classic.UploadCount,
                        source_generation = classic.SourceGeneration.ToString(),
                        source_tick = classic.SourceTick.ToString()
                    });

                    Debug.Log(
                        "OWNER CLASSIC HD VISUAL CAPTURE CASE PASS: route=" + routeIds[routeIndex] +
                        " file=" + fileName +
                        " frames=" + frameCounts[routeIndex] +
                        " width=" + classic.IndexTexture.width +
                        " height=" + classic.IndexTexture.height +
                        " upload_count=" + classic.UploadCount +
                        " resolved_hash=" + resolvedHash.ToString("x16") +
                        " png_sha256=" + Sha256Hex(pngBytes));
                }

                var receipt = new ClassicHdVisualCaptureReceipt
                {
                    schema_version = 1,
                    mode = "classic-hd",
                    profile = classic.Settings.profile,
                    capture_status = "captured-for-human-review",
                    visual_review_status = "not-reviewed",
                    owner_payload_embedded = false,
                    owner_inputs_external = true,
                    unity_editor = Application.unityVersion,
                    graphics_api = SystemInfo.graphicsDeviceType.ToString(),
                    graphics_device = SystemInfo.graphicsDeviceName,
                    graphics_driver = SystemInfo.graphicsDeviceVersion,
                    captures = entries.ToArray()
                };
                string receiptJson = JsonUtility.ToJson(receipt, true) + "\n";
                File.WriteAllText(
                    Path.Combine(captureDirectory, "classic-hd-owner-visual-capture-receipt.json"),
                    receiptJson);

                Debug.Log(
                    "OWNER CLASSIC HD VISUAL CAPTURE PROBE PASS: profile=" + classic.Settings.profile +
                    " captures=" + entries.Count +
                    " output_directory=" + captureDirectory +
                    " visual_review_status=not-reviewed" +
                    " owner_inputs_external=true");
                // Batchmode can begin process teardown immediately after Exit;
                // release the native-backed router before returning so the
                // owner session is not destroyed concurrently with Unity's
                // domain shutdown.
                UnityEngine.Object.DestroyImmediate(root);
                root = null;
                EditorApplication.Exit(0);
            }
            finally
            {
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Unity Enhanced Visual Capture Probe")]
        public static void RunOwnerUnityEnhancedVisualCaptureProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            string captureDirectory = CommandLineValue("-mc-capture-dir");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath) ||
                string.IsNullOrWhiteSpace(captureDirectory))
            {
                throw new InvalidOperationException(
                    "owner Unity Enhanced visual capture probe requires -mc-package, -mc-closure, -mc-asset-cache, and -mc-capture-dir");
            }

            captureDirectory = Path.GetFullPath(captureDirectory);
            Directory.CreateDirectory(captureDirectory);

            var root = new GameObject("Owner Unity Enhanced Visual Capture Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            string[] routeIds =
            {
                "town-approach",
                "town-overflight",
                "fireball-fight",
                "mana-castle",
                "reflective-water",
                "fog-depth",
                "high-speed-flight"
            };
            string[] captureNames =
            {
                "l1-night-town-approach",
                "l1-night-town-overflight",
                "l1-night-fireball-fight",
                "l1-night-mana-castle",
                "l1-night-reflective-water",
                "l1-night-fog-depth",
                "l1-night-high-speed-flight"
            };
            int[] frameCounts = { 8, 12, 12, 8, 12, 12, 16 };
            const int CaptureWidth = 1280;
            const int CaptureHeight = 720;
            var entries = new List<UnityEnhancedVisualCaptureEntry>();

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE FAIL: owner route did not open" +
                        " status=" + router.StatusLine +
                        " owner_validation=" + (router.OwnerValidation == null ? "null" : router.OwnerValidation.Reason));
                    EditorApplication.Exit(1);
                    return;
                }

                UnityEnhancedWorldRoot enhanced = router.EnhancedRoot;
                UnityEnhancedOwnerSessionSource source = router.EnhancedOwnerSession;
                string reason = string.Empty;
                for (int routeIndex = 0; routeIndex < routeIds.Length; routeIndex++)
                {
                    if (routeIndex > 0 && !router.ResetEnhancedOwnerSession(out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE FAIL: reset " +
                            routeIds[routeIndex] + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    bool castObserved = false;
                    for (int frame = 0; frame < frameCounts[routeIndex]; frame++)
                    {
                        Vector2 input = VisualCaptureInput(routeIds[routeIndex], frame);
                        bool cast = string.Equals(routeIds[routeIndex], "fireball-fight", StringComparison.Ordinal) &&
                                    frame == 4;
                        int spellSelection = cast ? 0 : -1;
                        if (!router.CaptureEnhancedFrame(
                                input.x,
                                input.y,
                                cast,
                                spellSelection,
                                out reason))
                        {
                            Debug.LogError(
                                "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE FAIL: route=" +
                                routeIds[routeIndex] + " frame=" + frame + ": " + reason);
                            EditorApplication.Exit(1);
                            return;
                        }

                        castObserved |= cast;
                    }

                    byte[] pngBytes;
                    ulong resolvedHash;
                    if (!enhanced.CapturePresentationFramePng(
                            CaptureWidth,
                            CaptureHeight,
                            out pngBytes,
                            out resolvedHash,
                            out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE FAIL: resolved capture route=" +
                            routeIds[routeIndex] + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    string fileName = captureNames[routeIndex] + ".png";
                    string outputPath = Path.Combine(captureDirectory, fileName);
                    string receiptPath = Path.Combine(
                        captureDirectory,
                        "unity-enhanced-owner-visual-capture-receipt.json");
                    if (File.Exists(outputPath) || (routeIndex == 0 && File.Exists(receiptPath)))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE FAIL: refusing to overwrite existing capture in " +
                            captureDirectory);
                        EditorApplication.Exit(1);
                        return;
                    }

                    File.WriteAllBytes(outputPath, pngBytes);
                    entries.Add(new UnityEnhancedVisualCaptureEntry
                    {
                        route_id = routeIds[routeIndex],
                        capture_name = captureNames[routeIndex],
                        file_name = fileName,
                        file_sha256 = Sha256Hex(pngBytes),
                        resolved_png_hash = resolvedHash.ToString("x16"),
                        frame_count = frameCounts[routeIndex],
                        cast_observed = castObserved,
                        width = CaptureWidth,
                        height = CaptureHeight,
                        active_entity_count = source.ActiveEntityCount,
                        active_visual_count = enhanced.ActorPresentation.ActiveVisualCount,
                        source_generation = source.Generation.ToString(),
                        source_tick = source.Tick.ToString()
                    });

                    Debug.Log(
                        "OWNER UNITY ENHANCED VISUAL CAPTURE CASE PASS: route=" + routeIds[routeIndex] +
                        " file=" + fileName +
                        " frames=" + frameCounts[routeIndex] +
                        " width=" + CaptureWidth +
                        " height=" + CaptureHeight +
                        " active_entities=" + source.ActiveEntityCount +
                        " active_visuals=" + enhanced.ActorPresentation.ActiveVisualCount +
                        " resolved_hash=" + resolvedHash.ToString("x16") +
                        " png_sha256=" + Sha256Hex(pngBytes));
                }

                var receipt = new UnityEnhancedVisualCaptureReceipt
                {
                    schema_version = 1,
                    mode = "unity-enhanced",
                    profile = "level1-night-unity-enhanced",
                    capture_status = "captured-for-human-review",
                    visual_review_status = "not-reviewed",
                    owner_payload_embedded = false,
                    owner_inputs_external = true,
                    unity_editor = Application.unityVersion,
                    graphics_api = SystemInfo.graphicsDeviceType.ToString(),
                    graphics_device = SystemInfo.graphicsDeviceName,
                    graphics_driver = SystemInfo.graphicsDeviceVersion,
                    captures = entries.ToArray()
                };
                string receiptJson = JsonUtility.ToJson(receipt, true) + "\n";
                File.WriteAllText(
                    Path.Combine(captureDirectory, "unity-enhanced-owner-visual-capture-receipt.json"),
                    receiptJson);

                Debug.Log(
                    "OWNER UNITY ENHANCED VISUAL CAPTURE PROBE PASS: captures=" + entries.Count +
                    " output_directory=" + captureDirectory +
                    " visual_review_status=not-reviewed" +
                    " owner_inputs_external=true");
                // Release the native-backed owner route before batchmode begins
                // domain teardown; destroying it from finally can race Unity's
                // native shutdown after a successful capture.
                source.Dispose();
                enhanced.Close();
                if (enhanced.gameObject != root)
                {
                    UnityEngine.Object.DestroyImmediate(enhanced.gameObject);
                }
                UnityEngine.Object.DestroyImmediate(root);
                root = null;
                EditorApplication.Exit(0);
            }
            finally
            {
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD High-Speed Probe")]
        public static void RunOwnerClassicHdHighSpeedProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD high-speed probe requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD High-Speed Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int FramesPerPass = 32;
            ulong[] firstIndexHashes = new ulong[FramesPerPass];
            ulong[] firstProvenanceHashes = new ulong[FramesPerPass];
            ulong[] secondIndexHashes = new ulong[FramesPerPass];
            ulong[] secondProvenanceHashes = new ulong[FramesPerPass];
            short maxTargetSpeed = short.MinValue;
            short maxActualSpeed = short.MinValue;
            byte maxEffectivePostprocessMode = 0;
            int automaticHighSpeedFrames = 0;
            int uploads = 0;
            bool resetReplayMatch = false;
            bool ownerSpeedSpellPresent = false;
            ulong firstTick = 0;
            ulong secondTick = 0;
            int allocationEpoch = 0;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError("OWNER CLASSIC HD HIGH SPEED PROBE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                if (!string.Equals(classic.Settings.blur, "off", StringComparison.OrdinalIgnoreCase))
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD HIGH SPEED PROBE FAIL: automatic probe requires -mc-classic-blur off" +
                        " actual=" + classic.Settings.blur);
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                ownerSpeedSpellPresent = classic.HasSpeedSpell;
                for (int pass = 0; pass < 2; pass++)
                {
                    if (pass == 1 && !classic.ResetOwnerSession(out reason))
                    {
                        Debug.LogError("OWNER CLASSIC HD HIGH SPEED PROBE FAIL: reset: " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    for (int frame = 0; frame < FramesPerPass; frame++)
                    {
                        Vector2 input = HighSpeedQualificationInput(frame);
                        if (!classic.CaptureFrame(input.x, input.y, false, -1, out reason))
                        {
                            Debug.LogError(
                                "OWNER CLASSIC HD HIGH SPEED PROBE FAIL: pass=" + pass +
                                " frame=" + frame + ": " + reason);
                            EditorApplication.Exit(1);
                            return;
                        }

                        if (pass == 0 && frame == 0)
                        {
                            firstTick = classic.SourceTick;
                        }
                        if (pass == 1 && frame == 0)
                        {
                            secondTick = classic.SourceTick;
                        }

                        maxTargetSpeed = Math.Max(maxTargetSpeed, classic.TargetSpeed);
                        maxActualSpeed = Math.Max(maxActualSpeed, (short)Math.Abs((int)classic.ActualSpeed));
                        maxEffectivePostprocessMode = Math.Max(
                            maxEffectivePostprocessMode,
                            classic.EffectivePostprocessMode);
                        if (Math.Abs((int)classic.ActualSpeed) > 80 &&
                            classic.EffectivePostprocessMode == 1)
                        {
                            automaticHighSpeedFrames++;
                        }

                        if (pass == 0)
                        {
                            firstIndexHashes[frame] = classic.LastNativeIndexHash;
                            firstProvenanceHashes[frame] = classic.LastNativeProvenanceHash;
                        }
                        else
                        {
                            secondIndexHashes[frame] = classic.LastNativeIndexHash;
                            secondProvenanceHashes[frame] = classic.LastNativeProvenanceHash;
                        }
                        uploads++;
                    }
                }

                allocationEpoch = classic.AllocationEpoch;
                resetReplayMatch = SequenceHash(firstIndexHashes) == SequenceHash(secondIndexHashes) &&
                    SequenceHash(firstProvenanceHashes) == SequenceHash(secondProvenanceHashes);
                bool routeContract = resetReplayMatch &&
                    uploads == FramesPerPass * 2 &&
                    allocationEpoch == 1;
                if (!routeContract)
                {
                    Debug.LogError(
                        "OWNER CLASSIC HD HIGH SPEED PROBE FAIL: reset/upload/allocation contract" +
                        " reset_replay_match=" + (resetReplayMatch ? 1 : 0) +
                        " uploads=" + uploads +
                        " allocation_epoch=" + allocationEpoch);
                    EditorApplication.Exit(1);
                    return;
                }

                if (automaticHighSpeedFrames > 0)
                {
                    Debug.Log(
                        "OWNER CLASSIC HD HIGH SPEED PROBE PASS: profile=" + classic.Settings.profile +
                        " frames_per_pass=" + FramesPerPass +
                        " pass_count=2 uploads=" + uploads +
                        " allocation_epoch=" + allocationEpoch +
                        " speed_spell_present=" + (ownerSpeedSpellPresent ? 1 : 0) +
                        " first_tick=" + firstTick +
                        " second_tick=" + secondTick +
                        " max_target_speed=" + maxTargetSpeed +
                        " max_actual_speed=" + maxActualSpeed +
                        " max_effective_postprocess_mode=" + maxEffectivePostprocessMode +
                        " automatic_high_speed_frames=" + automaticHighSpeedFrames +
                        " reset_replay_match=1" +
                        " index_route_hash=" + SequenceHash(firstIndexHashes).ToString("x16") +
                        " provenance_route_hash=" + SequenceHash(firstProvenanceHashes).ToString("x16") +
                        " gpu_readback_fence=not-captured");
                }
                else
                {
                    Debug.Log(
                        "OWNER CLASSIC HD HIGH SPEED PROBE OPEN: profile=" + classic.Settings.profile +
                        " frames_per_pass=" + FramesPerPass +
                        " pass_count=2 uploads=" + uploads +
                        " allocation_epoch=" + allocationEpoch +
                        " speed_spell_present=" + (ownerSpeedSpellPresent ? 1 : 0) +
                        " first_tick=" + firstTick +
                        " second_tick=" + secondTick +
                        " max_target_speed=" + maxTargetSpeed +
                        " max_actual_speed=" + maxActualSpeed +
                        " max_effective_postprocess_mode=" + maxEffectivePostprocessMode +
                        " automatic_high_speed_frames=0" +
                        " reset_replay_match=1" +
                        " index_route_hash=" + SequenceHash(firstIndexHashes).ToString("x16") +
                        " provenance_route_hash=" + SequenceHash(firstProvenanceHashes).ToString("x16") +
                        " reason=existing owner ABI exposes no legitimate Speed-jar activation; high-speed postprocess remains unqualified");
                }
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Completion Route")]
        public static void RunOwnerClassicHdCompletionRoute()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            string journalPath = CommandLineValue("-mc-command-journal");
            string receiptPath = CommandLineValue("-mc-completion-receipt");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath) ||
                string.IsNullOrWhiteSpace(journalPath) ||
                string.IsNullOrWhiteSpace(receiptPath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD completion route requires -mc-package, -mc-closure, " +
                    "-mc-asset-cache, -mc-command-journal, and -mc-completion-receipt");
            }

            var root = new GameObject("Owner Classic HD Completion Route Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            ClassicHdOwnerCommandJournal journal = null;
            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    throw new InvalidOperationException(
                        "owner Classic HD completion route did not open the owner-bound Rust session");
                }

                string reason;
                if (!ClassicHdOwnerCommandJournal.TryOpen(journalPath, out journal, out reason))
                {
                    throw new InvalidOperationException(reason);
                }

                if (!journal.MatchesExternalInputs(packagePath, closurePath, out reason))
                {
                    throw new InvalidOperationException(reason);
                }

                if (journal.CommandCount < 1_000)
                {
                    throw new InvalidOperationException(
                        "owner Classic HD completion journal is too short for a Level 1 route: " +
                        journal.CommandCount);
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                var indexHashes = new List<ulong>(journal.CommandCount);
                var provenanceHashes = new List<ulong>(journal.CommandCount);
                ulong initialGeneration = classic.SourceGeneration;
                ulong initialAuthoritativeGeneration = classic.AuthoritativeGeneration;
                short maxTargetSpeed = short.MinValue;
                short maxActualSpeed = short.MinValue;
                bool exitActiveObserved = false;

                for (int commandIndex = 0; commandIndex < journal.CommandCount; commandIndex++)
                {
                    ClassicHdOwnerCommand command;
                    if (!journal.ReadNext(out command, out reason))
                    {
                        throw new InvalidOperationException(reason);
                    }

                    if (!classic.CaptureOwnerCommand(command, out reason))
                    {
                        throw new InvalidOperationException(
                            "owner Classic HD completion command " + commandIndex + " failed: " + reason);
                    }

                    // The command advances the authoritative session first;
                    // the render-camera receipt is the pre-update presentation
                    // clock used to render that completed turn.
                    ulong expectedAuthoritativeTick = checked((ulong)commandIndex + 1UL);
                    ulong expectedSourceTick = checked((ulong)commandIndex);
                    if (classic.SourceGeneration != initialGeneration ||
                        classic.AuthoritativeGeneration != initialAuthoritativeGeneration ||
                        classic.AuthoritativeTick != expectedAuthoritativeTick ||
                        classic.SourceTick != expectedSourceTick)
                    {
                        throw new InvalidOperationException(
                            "owner Classic HD completion route changed generation or clock at command " +
                            commandIndex +
                            ": source_generation=" + classic.SourceGeneration +
                            " expected_generation=" + initialGeneration +
                            " authoritative_generation=" + classic.AuthoritativeGeneration +
                            " expected_authoritative_generation=" + initialAuthoritativeGeneration +
                            " authoritative_tick=" + classic.AuthoritativeTick +
                            " expected_authoritative_tick=" + expectedAuthoritativeTick +
                            " source_tick=" + classic.SourceTick +
                            " expected_source_tick=" + expectedSourceTick);
                    }

                    indexHashes.Add(classic.LastNativeIndexHash);
                    provenanceHashes.Add(classic.LastNativeProvenanceHash);
                    maxTargetSpeed = Math.Max(maxTargetSpeed, classic.TargetSpeed);
                    maxActualSpeed = Math.Max(maxActualSpeed, (short)Math.Abs((int)classic.ActualSpeed));
                    exitActiveObserved |= classic.ExitSequenceActive;
                    if ((commandIndex % 1024) == 0 || commandIndex + 1 == journal.CommandCount)
                    {
                        Debug.Log(
                            "OWNER CLASSIC HD COMPLETION ROUTE PROGRESS: command=" +
                            (commandIndex + 1) + "/" + journal.CommandCount +
                            " authoritative_tick=" + classic.AuthoritativeTick +
                            " source_tick=" + classic.SourceTick +
                            " stage_tasks=" + classic.TasksCompleted +
                            " exit_active=" + (classic.ExitSequenceActive ? 1 : 0) +
                            " exit_complete=" + (classic.ExitSequenceComplete ? 1 : 0));
                    }
                }

                if (journal.ReadCount != journal.CommandCount)
                {
                    throw new InvalidOperationException(
                        "owner Classic HD completion route did not consume the full command journal");
                }

                // Completion transitions the native state out of its transient
                // active phase. Keep the observed-active latch as the proof
                // that the route entered the authored exit sequence.
                bool terminal = exitActiveObserved &&
                    classic.ExitSequenceComplete &&
                    classic.ExitResultCode == 10 &&
                    classic.TasksCompleted > 0;
                string status = terminal ? "pass" : "open";
                string routeReason = terminal
                    ? "owner journal reached the authored exit result"
                    : "owner journal replay completed without the qualified exit result";
                var receipt = new ClassicHdOwnerCompletionReceipt
                {
                    schema_version = 1,
                    mode = "classic-hd",
                    route = "level1-completion-postgame",
                    status = status,
                    evidence_authority = "owner-external-input-journal",
                    owner_payload_embedded = false,
                    owner_inputs_external = true,
                    journal_schema = journal.ManifestSchema,
                    journal_status = journal.ManifestStatus,
                    journal_command_encoding = journal.ManifestCommandEncoding,
                    journal_command_count = journal.CommandCount,
                    journal_replayed_commands = journal.ReadCount,
                    journal_manifest_sha256 = Sha256File(journal.ManifestPath),
                    journal_commands_sha256 = Sha256File(journal.CommandsPath),
                    frame_count = indexHashes.Count,
                    upload_count = classic.UploadCount,
                    allocation_epoch = classic.AllocationEpoch,
                    generation = classic.SourceGeneration,
                    initial_tick = 0,
                    final_tick = classic.AuthoritativeTick,
                    source_initial_tick = 0,
                    source_final_tick = classic.SourceTick,
                    authoritative_generation = classic.AuthoritativeGeneration,
                    generation_stable = classic.SourceGeneration == initialGeneration &&
                        classic.AuthoritativeGeneration == initialAuthoritativeGeneration,
                    exit_sequence_active_observed = exitActiveObserved,
                    exit_sequence_active = classic.ExitSequenceActive,
                    exit_sequence_complete = classic.ExitSequenceComplete,
                    exit_result_code = classic.ExitResultCode,
                    tasks_completed = classic.TasksCompleted,
                    max_target_speed = maxTargetSpeed,
                    max_actual_speed = maxActualSpeed,
                    index_route_hash = SequenceHash(indexHashes).ToString("x16"),
                    provenance_route_hash = SequenceHash(provenanceHashes).ToString("x16"),
                    reason = routeReason
                };
                string absoluteReceiptPath = Path.GetFullPath(receiptPath);
                string receiptDirectory = Path.GetDirectoryName(absoluteReceiptPath);
                if (!string.IsNullOrEmpty(receiptDirectory))
                {
                    Directory.CreateDirectory(receiptDirectory);
                }
                File.WriteAllText(
                    absoluteReceiptPath,
                    JsonUtility.ToJson(receipt, true) + "\n");

                Debug.Log(
                    "OWNER CLASSIC HD COMPLETION ROUTE " + status.ToUpperInvariant() +
                    ": command_count=" + journal.CommandCount +
                    " frame_count=" + receipt.frame_count +
                    " uploads=" + receipt.upload_count +
                    " generation=" + receipt.generation +
                    " final_tick=" + receipt.final_tick +
                    " exit_complete=" + (receipt.exit_sequence_complete ? 1 : 0) +
                    " result=" + receipt.exit_result_code +
                    " receipt=" + absoluteReceiptPath +
                    " reason=" + routeReason.Replace(' ', '_'));
                EditorApplication.Exit(0);
            }
            catch (Exception exception)
            {
                Debug.LogError("OWNER CLASSIC HD COMPLETION ROUTE FAIL: " + exception);
                EditorApplication.Exit(1);
            }
            finally
            {
                if (journal != null)
                {
                    journal.Dispose();
                }
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD Performance Smoke")]
        public static void RunOwnerClassicHdPerformanceSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD performance smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD Performance Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int WarmupFrames = 8;
            const int MeasurementFrames = 64;
            const long PresentationBudgetNanoseconds = 33333333L;
            var nativeNanoseconds = new List<long>(MeasurementFrames);
            var uploadNanoseconds = new List<long>(MeasurementFrames);
            var totalNanoseconds = new List<long>(MeasurementFrames);
            var allocationBytes = new List<long>(MeasurementFrames);
            var indexHashes = new List<ulong>(MeasurementFrames);
            var provenanceHashes = new List<ulong>(MeasurementFrames);
            ulong startGeneration = 0;
            ulong endGeneration = 0;
            ulong startTick = 0;
            ulong endTick = 0;
            ulong previousTick = 0;
            int uploads = 0;
            bool monotonic = true;
            bool hashesPresent = true;
            bool warmupComplete = false;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError("OWNER CLASSIC HD PERFORMANCE SMOKE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                string reason;
                int startUploads = classic.UploadCount;
                for (int frame = 0; frame < WarmupFrames; frame++)
                {
                    Vector2 input = PerformanceQualificationInput(frame);
                    if (!classic.CaptureFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD PERFORMANCE SMOKE FAIL: warmup frame=" + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }
                }
                warmupComplete = true;

                for (int frame = 0; frame < MeasurementFrames; frame++)
                {
                    Vector2 input = PerformanceQualificationInput(WarmupFrames + frame);
                    long allocatedBefore = GC.GetAllocatedBytesForCurrentThread();
                    long totalStart = System.Diagnostics.Stopwatch.GetTimestamp();
                    if (!classic.CaptureFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError(
                            "OWNER CLASSIC HD PERFORMANCE SMOKE FAIL: measurement frame=" + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }
                    long totalElapsed = ElapsedNanoseconds(totalStart);
                    long allocatedAfter = GC.GetAllocatedBytesForCurrentThread();

                    if (frame == 0)
                    {
                        startGeneration = classic.SourceGeneration;
                        startTick = classic.SourceTick;
                        previousTick = classic.SourceTick;
                    }
                    else if (classic.SourceGeneration != startGeneration ||
                             classic.SourceTick != previousTick + 1)
                    {
                        monotonic = false;
                    }
                    previousTick = classic.SourceTick;
                    endGeneration = classic.SourceGeneration;
                    endTick = classic.SourceTick;

                    nativeNanoseconds.Add(classic.LastNativeFrameNanoseconds);
                    uploadNanoseconds.Add(classic.LastUploadNanoseconds);
                    totalNanoseconds.Add(totalElapsed);
                    allocationBytes.Add(allocatedAfter - allocatedBefore);
                    indexHashes.Add(classic.LastNativeIndexHash);
                    provenanceHashes.Add(classic.LastNativeProvenanceHash);
                    hashesPresent = hashesPresent &&
                        classic.LastNativeIndexHash != 0 &&
                        classic.LastNativeProvenanceHash != 0;
                }

                uploads = classic.UploadCount - startUploads;
                long p95Native = PercentileNanoseconds(nativeNanoseconds, 0.95);
                long p95Upload = PercentileNanoseconds(uploadNanoseconds, 0.95);
                long p95Total = PercentileNanoseconds(totalNanoseconds, 0.95);
                long maxNative = MaxValue(nativeNanoseconds);
                long maxUpload = MaxValue(uploadNanoseconds);
                long maxTotal = MaxValue(totalNanoseconds);
                long p95Allocation = PercentileNanoseconds(allocationBytes, 0.95);
                long maxAllocation = MaxValue(allocationBytes);
                bool steadyAllocations = p95Allocation == 0 && maxAllocation == 0;
                bool routeContract = warmupComplete &&
                    monotonic &&
                    hashesPresent &&
                    uploads == WarmupFrames + MeasurementFrames &&
                    classic.AllocationEpoch == 1 &&
                    endGeneration == startGeneration &&
                    endTick == startTick + MeasurementFrames - 1;
                bool budgetPass = p95Total <= PresentationBudgetNanoseconds;
                bool pass = routeContract && steadyAllocations && budgetPass;
                string status = pass ? "PASS" : "OPEN";
                Debug.Log(
                    "OWNER CLASSIC HD PERFORMANCE SMOKE " + status + ":" +
                    " profile=" + classic.Settings.profile +
                    " warmup_frames=" + WarmupFrames +
                    " measurement_frames=" + MeasurementFrames +
                    " uploads=" + uploads +
                    " allocation_epoch=" + classic.AllocationEpoch +
                    " start_generation=" + startGeneration +
                    " end_generation=" + endGeneration +
                    " start_tick=" + startTick +
                    " end_tick=" + endTick +
                    " monotonic=" + (monotonic ? 1 : 0) +
                    " hashes_present=" + (hashesPresent ? 1 : 0) +
                    " p95_native_ns=" + p95Native +
                    " max_native_ns=" + maxNative +
                    " p95_upload_ns=" + p95Upload +
                    " max_upload_ns=" + maxUpload +
                    " p95_total_ns=" + p95Total +
                    " max_total_ns=" + maxTotal +
                    " presentation_budget_ns=" + PresentationBudgetNanoseconds +
                    " p95_allocation_bytes=" + p95Allocation +
                    " max_allocation_bytes=" + maxAllocation +
                    " steady_allocations=" + (steadyAllocations ? 1 : 0) +
                    " route_contract=" + (routeContract ? 1 : 0) +
                    " budget_pass=" + (budgetPass ? 1 : 0) +
                    " index_route_hash=" + SequenceHash(indexHashes).ToString("x16") +
                    " provenance_route_hash=" + SequenceHash(provenanceHashes).ToString("x16") +
                    " graphics_api=" + SystemInfo.graphicsDeviceType +
                    " graphics_device=" + SystemInfo.graphicsDeviceName +
                    " execution_context=UnityEditor-batchmode" +
                    " gpu_timing=not-captured" +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Classic HD GPU Fence Probe")]
        public static void RunOwnerClassicHdGpuFenceProbe()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Classic HD GPU fence probe requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Classic HD GPU Fence Probe Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.ClassicHdRenderer == null ||
                    !router.ClassicHdRenderer.UsesNativeOwnerSession)
                {
                    Debug.LogError("OWNER CLASSIC HD GPU FENCE PROBE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                ClassicHdPresentation classic = router.ClassicHdRenderer;
                if (classic.IndexTexture == null || classic.IndexedMaterial == null)
                {
                    Debug.LogError("OWNER CLASSIC HD GPU FENCE PROBE FAIL: indexed surface/material is missing");
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                int startUploads = classic.UploadCount;
                ulong rawHashA;
                ulong resolvedHashA;
                bool fenceValidA;
                bool fencePassedA;
                bool rawErrorA;
                bool resolvedErrorA;
                string readbackReasonA = string.Empty;
                if (!classic.CaptureFrame(0.0f, 0.0f, false, -1, out reason) ||
                    !ReadGpuHashes(
                        classic,
                        out rawHashA,
                        out resolvedHashA,
                        out fenceValidA,
                        out fencePassedA,
                        out rawErrorA,
                        out resolvedErrorA,
                        out readbackReasonA))
                {
                    Debug.Log(
                        "OWNER CLASSIC HD GPU FENCE PROBE OPEN: profile=" + classic.Settings.profile +
                        " reason=" + (string.IsNullOrWhiteSpace(readbackReasonA) ? reason : readbackReasonA) +
                        " execution_context=UnityEditor-batchmode gpu_timing=not-captured");
                    EditorApplication.Exit(0);
                    return;
                }
                long nativeFrameNanosecondsA = classic.LastNativeFrameNanoseconds;
                long uploadSubmissionNanosecondsA = classic.LastUploadNanoseconds;

                ulong rawHashB;
                ulong resolvedHashB;
                bool fenceValidB;
                bool fencePassedB;
                bool rawErrorB;
                bool resolvedErrorB;
                string readbackReasonB = string.Empty;
                if (!classic.CaptureFrame(0.30f, 0.20f, false, -1, out reason) ||
                    !ReadGpuHashes(
                        classic,
                        out rawHashB,
                        out resolvedHashB,
                        out fenceValidB,
                        out fencePassedB,
                        out rawErrorB,
                        out resolvedErrorB,
                        out readbackReasonB))
                {
                    Debug.Log(
                        "OWNER CLASSIC HD GPU FENCE PROBE OPEN: profile=" + classic.Settings.profile +
                        " reason=" + (string.IsNullOrWhiteSpace(readbackReasonB) ? reason : readbackReasonB) +
                        " execution_context=UnityEditor-batchmode gpu_timing=not-captured");
                    EditorApplication.Exit(0);
                    return;
                }
                long nativeFrameNanosecondsB = classic.LastNativeFrameNanoseconds;
                long uploadSubmissionNanosecondsB = classic.LastUploadNanoseconds;

                int uploads = classic.UploadCount - startUploads;
                bool hashesDiffer = rawHashA != rawHashB && resolvedHashA != resolvedHashB;
                bool readbackOk = !rawErrorA && !resolvedErrorA && !rawErrorB && !resolvedErrorB;
                bool fenceOk = fenceValidA && fencePassedA && fenceValidB && fencePassedB;
                bool contract = hashesDiffer && readbackOk && fenceOk && uploads == 2 && classic.AllocationEpoch == 1;
                string common =
                    " profile=" + classic.Settings.profile +
                    " frames=2 uploads=" + uploads +
                    " allocation_epoch=" + classic.AllocationEpoch +
                    " raw_hash_a=" + rawHashA.ToString("x16") +
                    " raw_hash_b=" + rawHashB.ToString("x16") +
                    " resolved_hash_a=" + resolvedHashA.ToString("x16") +
                    " resolved_hash_b=" + resolvedHashB.ToString("x16") +
                    " native_frame_ns_a=" + nativeFrameNanosecondsA +
                    " native_frame_ns_b=" + nativeFrameNanosecondsB +
                    " upload_submission_ns_a=" + uploadSubmissionNanosecondsA +
                    " upload_submission_ns_b=" + uploadSubmissionNanosecondsB +
                    " fence_valid_a=" + (fenceValidA ? 1 : 0) +
                    " fence_passed_a=" + (fencePassedA ? 1 : 0) +
                    " fence_valid_b=" + (fenceValidB ? 1 : 0) +
                    " fence_passed_b=" + (fencePassedB ? 1 : 0) +
                    " readback_error_a=" + ((rawErrorA || resolvedErrorA) ? 1 : 0) +
                    " readback_error_b=" + ((rawErrorB || resolvedErrorB) ? 1 : 0) +
                    " hashes_differ=" + (hashesDiffer ? 1 : 0) +
                    " execution_context=UnityEditor-batchmode" +
                    " graphics_api=" + SystemInfo.graphicsDeviceType +
                    " graphics_device=" + SystemInfo.graphicsDeviceName +
                    " gpu_timing=not-captured";
                if (contract)
                {
                    Debug.Log("OWNER CLASSIC HD GPU FENCE PROBE PASS:" + common + " status=readback-fence-pass");
                }
                else
                {
                    Debug.Log(
                        "OWNER CLASSIC HD GPU FENCE PROBE OPEN:" + common +
                        " status=readback-or-fence-incomplete reason=" +
                        (string.IsNullOrWhiteSpace(readbackReasonB) ? readbackReasonA : readbackReasonB));
                }
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Unity Enhanced Sequence Smoke")]
        public static void RunOwnerUnityEnhancedSequenceSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Unity Enhanced sequence smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Unity Enhanced Sequence Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int FramesPerPass = 32;
            const int TerrainMutationFrame = 12;
            const ushort TerrainQualificationHouse = 11;
            const ushort TerrainQualificationParent = 152;
            const byte TerrainQualificationSelector = 17;
            ulong[] firstStateHashes = new ulong[FramesPerPass];
            ulong[] firstTerrainHashes = new ulong[FramesPerPass];
            ulong[] firstTerrainMutationSequences = new ulong[FramesPerPass];
            ulong[] firstWorldHashes = new ulong[FramesPerPass];
            int[] firstEventCounts = new int[FramesPerPass];
            int[] firstVfxCounts = new int[FramesPerPass];
            int[] firstActiveEntityCounts = new int[FramesPerPass];
            int[] firstActiveVisualCounts = new int[FramesPerPass];
            int[] firstTerrainApplications = new int[FramesPerPass];
            bool[] firstTerrainPublished = new bool[FramesPerPass];
            bool[] firstTerrainMutationPublished = new bool[FramesPerPass];
            ulong firstGeneration;
            ulong firstTick;
            ulong secondGeneration;
            ulong secondTick;
            bool sawOwnerEvent = false;
            bool sawOwnerVfx = false;
            bool sawDynamicEffectRows = false;
            bool sawDynamicActorVisual = false;
            bool sawTerrainMutationRebuild = false;
            bool sawTerrainNoopStep = false;
            int terrainMutationRebuilds = 0;
            int maxMissionProjectiles = 0;
            int maxMissionImpacts = 0;
            int maxEventRows = 0;
            int maxVfxRows = 0;
            int maxActiveVfx = 0;
            int maxPresentationVisuals = 0;
            int maxPooledVisuals = 0;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null ||
                    router.EnhancedRoot.State != UnityEnhancedLifecycleState.Open)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: router did not open the owner-bound Enhanced route");
                    EditorApplication.Exit(1);
                    return;
                }

                UnityEnhancedWorldRoot resetRoot = router.EnhancedRoot;
                UnityEnhancedOwnerSessionSource resetSource = router.EnhancedOwnerSession;
                if (resetRoot.LastTick != 0 || resetSource.Tick != 0 ||
                    resetSource.PresentationTick != 0 || resetSource.HasPendingResetSnapshot ||
                    !resetRoot.TerrainPresentation.HasSnapshot ||
                    !resetRoot.WaterPresentation.HasSnapshot ||
                    resetRoot.ActorPresentation.ActiveVisualCount == 0)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: explicit tick-0 reset was not applied " +
                        "root_tick=" + resetRoot.LastTick +
                        " source_tick=" + resetSource.Tick +
                        " presentation_tick=" + resetSource.PresentationTick +
                        " pending_reset=" + resetSource.HasPendingResetSnapshot);
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                int resetActiveVisuals = resetRoot.ActorPresentation.ActiveVisualCount;
                ulong previousTerrainMutationSequence = resetSource.TerrainMutationSequence;
                ulong previousTerrainHash = resetSource.LastTerrainHash;
                int previousTerrainApplications =
                    resetRoot.TerrainPresentation.DirtyRegionApplications;
                for (int frame = 0; frame < FramesPerPass; frame++)
                {
                    if (frame == TerrainMutationFrame &&
                        !router.EnhancedOwnerSession.PrepareTerrainMutationQualification(
                            TerrainQualificationHouse,
                            TerrainQualificationParent,
                            TerrainQualificationSelector,
                            out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: terrain mutation setup at frame " +
                            frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    Vector2 input = SequenceInput(frame);
                    bool cast = frame == 4;
                    int spellSelection = frame == 3 ? 0 : -1;
                    if (!router.CaptureEnhancedFrame(
                            input.x,
                            input.y,
                            cast,
                            spellSelection,
                            out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: first pass frame " +
                            frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    UnityEnhancedWorldRoot enhanced = router.EnhancedRoot;
                    UnityEnhancedOwnerSessionSource source = router.EnhancedOwnerSession;
                    if (enhanced.State != UnityEnhancedLifecycleState.Open ||
                        !enhanced.TerrainPresentation.HasSnapshot ||
                        !enhanced.WaterPresentation.HasSnapshot ||
                        !enhanced.NightLighting.HasDescriptor ||
                        enhanced.ActorPresentation.ActiveVisualCount == 0 ||
                        source.ActiveEntityCount == 0 ||
                        source.PresentationTick + 1 != source.Tick)
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: incomplete live snapshot at frame " +
                            frame + " tick=" + source.Tick +
                            " presentation_tick=" + source.PresentationTick +
                            " active_entities=" + source.ActiveEntityCount +
                            " active_visuals=" + enhanced.ActorPresentation.ActiveVisualCount);
                        EditorApplication.Exit(1);
                        return;
                    }

                    firstStateHashes[frame] = source.LastStateHash;
                    firstTerrainHashes[frame] = source.LastTerrainHash;
                    firstTerrainMutationSequences[frame] = source.TerrainMutationSequence;
                    firstWorldHashes[frame] = source.LastWorldHash;
                    firstEventCounts[frame] = source.LastEventCount;
                    firstVfxCounts[frame] = source.LastVfxEventCount;
                    firstActiveEntityCounts[frame] = source.ActiveEntityCount;
                    firstActiveVisualCounts[frame] = enhanced.ActorPresentation.ActiveVisualCount;
                    firstTerrainApplications[frame] = enhanced.TerrainPresentation.DirtyRegionApplications;
                    firstTerrainPublished[frame] = source.LastTerrainSnapshotPublished;
                    firstTerrainMutationPublished[frame] = source.LastTerrainMutationPublished;
                    sawOwnerEvent |= source.LastEventCount > 0;
                    sawOwnerVfx |= source.LastVfxEventCount > 0 &&
                                   enhanced.VfxPresentation.ActiveEffectCount > 0;
                    sawDynamicEffectRows |= source.MissionProjectileCount > 0 ||
                                            source.MissionImpactCount > 0;
                    sawDynamicActorVisual |= enhanced.ActorPresentation.ActiveVisualCount > resetActiveVisuals;
                    maxMissionProjectiles = Math.Max(maxMissionProjectiles, source.MissionProjectileCount);
                    maxMissionImpacts = Math.Max(maxMissionImpacts, source.MissionImpactCount);
                    maxEventRows = Math.Max(maxEventRows, source.LastEventCount);
                    maxVfxRows = Math.Max(maxVfxRows, source.LastVfxEventCount);
                    maxActiveVfx = Math.Max(maxActiveVfx, enhanced.VfxPresentation.ActiveEffectCount);
                    maxPresentationVisuals = Math.Max(
                        maxPresentationVisuals,
                        enhanced.ActorPresentation.ActiveVisualCount);
                    maxPooledVisuals = Math.Max(
                        maxPooledVisuals,
                        enhanced.ActorPresentation.PooledVisualCount);
                    if (firstStateHashes[frame] == 0 ||
                        firstTerrainHashes[frame] == 0 ||
                        firstWorldHashes[frame] == 0)
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: empty native hash at frame " + frame);
                        EditorApplication.Exit(1);
                        return;
                    }

                    bool terrainMutationChanged =
                        source.TerrainMutationSequence != previousTerrainMutationSequence;
                    if (source.LastTerrainMutationPublished != terrainMutationChanged)
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: terrain mutation publication " +
                            "did not match the native mutation sequence at frame " + frame +
                            " published=" + source.LastTerrainMutationPublished +
                            " changed=" + terrainMutationChanged);
                        EditorApplication.Exit(1);
                        return;
                    }

                    if (terrainMutationChanged)
                    {
                        if (!source.LastTerrainSnapshotPublished || !source.LastTerrainFullUpdate ||
                            source.LastTerrainHash == previousTerrainHash ||
                            enhanced.TerrainPresentation.DirtyRegionApplications !=
                                previousTerrainApplications + 1)
                        {
                            Debug.LogError(
                                "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: terrain mutation was not " +
                                "published as one rebuild at frame " + frame +
                                " published=" + source.LastTerrainSnapshotPublished +
                                " full_update=" + source.LastTerrainFullUpdate +
                                " mutation=" + source.TerrainMutationSequence + "/" +
                                previousTerrainMutationSequence +
                                " hash=" + source.LastTerrainHash.ToString("x16") + "/" +
                                previousTerrainHash.ToString("x16") +
                                " applications=" + enhanced.TerrainPresentation.DirtyRegionApplications +
                                "/" + (previousTerrainApplications + 1));
                            EditorApplication.Exit(1);
                            return;
                        }

                        terrainMutationRebuilds++;
                        if (frame == TerrainMutationFrame)
                        {
                            sawTerrainMutationRebuild = true;
                        }
                    }

                    if (frame > TerrainMutationFrame && !terrainMutationChanged &&
                        !source.LastTerrainSnapshotPublished && !source.LastTerrainOriginChanged &&
                        enhanced.TerrainPresentation.DirtyRegionApplications == previousTerrainApplications)
                    {
                        sawTerrainNoopStep = true;
                    }

                    previousTerrainMutationSequence = source.TerrainMutationSequence;
                    previousTerrainHash = source.LastTerrainHash;
                    previousTerrainApplications = enhanced.TerrainPresentation.DirtyRegionApplications;
                }

                UnityEnhancedOwnerSessionSource firstSource = router.EnhancedOwnerSession;
                if (!sawOwnerEvent || !sawOwnerVfx || !sawDynamicEffectRows || !sawDynamicActorVisual ||
                    !sawTerrainMutationRebuild || !sawTerrainNoopStep ||
                    firstSource.EventHead == 0 || firstSource.EventCursor != firstSource.EventHead)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: owner event/effect seam was not observable " +
                        "saw_event=" + sawOwnerEvent +
                        " saw_vfx=" + sawOwnerVfx +
                        " saw_dynamic_rows=" + sawDynamicEffectRows +
                        " saw_dynamic_actor=" + sawDynamicActorVisual +
                        " saw_terrain_rebuild=" + sawTerrainMutationRebuild +
                        " saw_terrain_noop=" + sawTerrainNoopStep +
                        " event_cursor=" + firstSource.EventCursor +
                        " event_head=" + firstSource.EventHead);
                    EditorApplication.Exit(1);
                    return;
                }

                firstGeneration = router.EnhancedOwnerSession.Generation;
                firstTick = router.EnhancedOwnerSession.Tick;
                ulong firstStateSequenceHash = SequenceHash(firstStateHashes);
                ulong firstTerrainSequenceHash = SequenceHash(firstTerrainHashes);
                ulong firstWorldSequenceHash = SequenceHash(firstWorldHashes);

                if (!router.ResetEnhancedOwnerSession(out reason))
                {
                    Debug.LogError("OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: reset: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                for (int frame = 0; frame < FramesPerPass; frame++)
                {
                    if (frame == TerrainMutationFrame &&
                        !router.EnhancedOwnerSession.PrepareTerrainMutationQualification(
                            TerrainQualificationHouse,
                            TerrainQualificationParent,
                            TerrainQualificationSelector,
                            out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: reset terrain mutation setup at frame " +
                            frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    Vector2 input = SequenceInput(frame);
                    bool cast = frame == 4;
                    int spellSelection = frame == 3 ? 0 : -1;
                    if (!router.CaptureEnhancedFrame(
                            input.x,
                            input.y,
                            cast,
                            spellSelection,
                            out reason))
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: reset pass frame " +
                            frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    UnityEnhancedOwnerSessionSource source = router.EnhancedOwnerSession;
                    if (source.LastStateHash != firstStateHashes[frame] ||
                        source.LastTerrainHash != firstTerrainHashes[frame] ||
                        source.TerrainMutationSequence != firstTerrainMutationSequences[frame] ||
                        source.LastWorldHash != firstWorldHashes[frame] ||
                        source.LastEventCount != firstEventCounts[frame] ||
                        source.LastVfxEventCount != firstVfxCounts[frame] ||
                        source.ActiveEntityCount != firstActiveEntityCounts[frame] ||
                        router.EnhancedRoot.ActorPresentation.ActiveVisualCount != firstActiveVisualCounts[frame] ||
                        router.EnhancedRoot.TerrainPresentation.DirtyRegionApplications !=
                            firstTerrainApplications[frame] ||
                        source.LastTerrainSnapshotPublished != firstTerrainPublished[frame] ||
                        source.LastTerrainMutationPublished != firstTerrainMutationPublished[frame])
                    {
                        Debug.LogError(
                            "OWNER UNITY ENHANCED SEQUENCE SMOKE FAIL: reset replay diverged at frame " +
                            frame + " first=" + firstStateHashes[frame].ToString("x16") + "/" +
                            firstTerrainHashes[frame].ToString("x16") + "/" +
                            firstWorldHashes[frame].ToString("x16") + " second=" +
                            source.LastStateHash.ToString("x16") + "/" +
                            source.LastTerrainHash.ToString("x16") + "/" +
                            source.LastWorldHash.ToString("x16") + " events=" +
                            source.LastEventCount + "/" + firstEventCounts[frame] +
                            " vfx=" + source.LastVfxEventCount + "/" + firstVfxCounts[frame]);
                        EditorApplication.Exit(1);
                        return;
                    }
                }

                secondGeneration = router.EnhancedOwnerSession.Generation;
                secondTick = router.EnhancedOwnerSession.Tick;
                Debug.Log(
                    "OWNER UNITY ENHANCED SEQUENCE SMOKE PASS: frames_per_pass=" + FramesPerPass +
                    " reset_tick=0" +
                    " first_generation=" + firstGeneration +
                    " first_tick=" + firstTick +
                    " second_generation=" + secondGeneration +
                    " second_tick=" + secondTick +
                    " active_entities=" + router.EnhancedOwnerSession.ActiveEntityCount +
                    " active_visuals=" + router.EnhancedRoot.ActorPresentation.ActiveVisualCount +
                    " event_head=" + router.EnhancedOwnerSession.EventHead +
                    " max_event_rows=" + maxEventRows +
                    " max_vfx_rows=" + maxVfxRows +
                    " max_active_vfx=" + maxActiveVfx +
                    " max_presentation_visuals=" + maxPresentationVisuals +
                    " max_pooled_visuals=" + maxPooledVisuals +
                    " terrain_mutation_frame=" + TerrainMutationFrame +
                    " terrain_mutation_sequence=" + router.EnhancedOwnerSession.TerrainMutationSequence +
                    " terrain_mutation_rebuilds=" + terrainMutationRebuilds +
                    " terrain_noop_steps_after_mutation=" + (sawTerrainNoopStep ? 1 : 0) +
                    " max_projectiles=" + maxMissionProjectiles +
                    " max_impacts=" + maxMissionImpacts +
                    " dynamic_projectiles=" + router.EnhancedOwnerSession.MissionProjectileCount +
                    " dynamic_impacts=" + router.EnhancedOwnerSession.MissionImpactCount +
                    " terrain_vertices=" + router.EnhancedRoot.TerrainPresentation.VertexCount +
                    " shoreline_cells=" + router.EnhancedRoot.WaterPresentation.ShorelineCellCount +
                    " state_sequence=" + firstStateSequenceHash.ToString("x16") +
                    " terrain_sequence=" + firstTerrainSequenceHash.ToString("x16") +
                    " world_sequence=" + firstWorldSequenceHash.ToString("x16") +
                    " lighting=" + router.EnhancedRoot.NightLighting.HasDescriptor +
                    " reflection=" + router.EnhancedRoot.WaterPresentation.ReflectionEnabled);
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Unity Enhanced Scenario Smoke")]
        public static void RunOwnerUnityEnhancedScenarioSmoke()
        {
            string packagePath = CommandLineValue("-mc-package");
            string closurePath = CommandLineValue("-mc-closure");
            string assetCachePath = CommandLineValue("-mc-asset-cache");
            if (string.IsNullOrWhiteSpace(packagePath) ||
                string.IsNullOrWhiteSpace(closurePath) ||
                string.IsNullOrWhiteSpace(assetCachePath))
            {
                throw new InvalidOperationException(
                    "owner Unity Enhanced scenario smoke requires -mc-package, -mc-closure, and -mc-asset-cache");
            }

            var root = new GameObject("Owner Unity Enhanced Scenario Smoke Router");
            var router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            const int IdleFrames = 8;
            const int TownFrames = 12;
            const int CombatFrames = 12;
            const int FlightFrames = 16;
            var stateHashes = new List<ulong>();
            var eventHeads = new List<ulong>();
            int townEvents = 0;
            int combatEvents = 0;
            int combatProjectiles = 0;
            int combatImpacts = 0;
            short maxFlightSpeed = short.MinValue;
            float flightCameraDistance = 0f;
            bool deathObserved = false;
            ulong generationBeforeRestart = 0;
            ulong generationAfterRestart = 0;
            ulong restartTick = ulong.MaxValue;

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: owner route did not open");
                    EditorApplication.Exit(1);
                    return;
                }

                string reason;
                UnityEnhancedOwnerSessionSource source = router.EnhancedOwnerSession;
                UnityEnhancedWorldRoot enhanced = router.EnhancedRoot;
                Vector3 townStart = source.CameraPosition;
                Vector3 flightStart = Vector3.zero;

                for (int frame = 0; frame < IdleFrames; frame++)
                {
                    if (!router.CaptureEnhancedFrame(0f, 0f, false, -1, out reason))
                    {
                        Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: idle frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    stateHashes.Add(source.LastStateHash);
                    eventHeads.Add(source.EventHead);
                }

                townStart = source.CameraPosition;
                for (int frame = 0; frame < TownFrames; frame++)
                {
                    Vector2 input = ScenarioFlightInput("town", frame);
                    if (!router.CaptureEnhancedFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: town frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    townEvents += source.LastEventCount;
                    stateHashes.Add(source.LastStateHash);
                    eventHeads.Add(source.EventHead);
                }

                if (source.ActiveEntityCount <= 0 ||
                    Vector3.Distance(townStart, source.CameraPosition) <= 0.001f)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: town route did not publish a moving populated view");
                    EditorApplication.Exit(1);
                    return;
                }

                for (int frame = 0; frame < CombatFrames; frame++)
                {
                    bool cast = frame == 4;
                    int spellSelection = frame == 3 ? 0 : -1;
                    if (!router.CaptureEnhancedFrame(0f, 0f, cast, spellSelection, out reason))
                    {
                        Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: combat frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    combatEvents += source.LastEventCount;
                    combatProjectiles = Math.Max(combatProjectiles, source.MissionProjectileCount);
                    combatImpacts = Math.Max(combatImpacts, source.MissionImpactCount);
                    stateHashes.Add(source.LastStateHash);
                    eventHeads.Add(source.EventHead);
                }

                if (combatEvents <= 0 || combatProjectiles <= 0 || combatImpacts <= 0)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: combat route did not publish event/projectile/impact observations" +
                        " events=" + combatEvents +
                        " projectiles=" + combatProjectiles +
                        " impacts=" + combatImpacts);
                    EditorApplication.Exit(1);
                    return;
                }

                flightStart = source.CameraPosition;
                for (int frame = 0; frame < FlightFrames; frame++)
                {
                    Vector2 input = ScenarioFlightInput("flight", frame);
                    if (!router.CaptureEnhancedFrame(input.x, input.y, false, -1, out reason))
                    {
                        Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: flight frame " + frame + ": " + reason);
                        EditorApplication.Exit(1);
                        return;
                    }

                    maxFlightSpeed = Math.Max(maxFlightSpeed, source.ActualSpeed);
                    stateHashes.Add(source.LastStateHash);
                    eventHeads.Add(source.EventHead);
                }

                flightCameraDistance = Vector3.Distance(
                    flightStart,
                    source.CameraPosition);
                if (maxFlightSpeed <= 0 && flightCameraDistance <= 0.001f)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: high-speed flight route did not move the authoritative camera");
                    EditorApplication.Exit(1);
                    return;
                }

                if (!source.InjectPlayerDamageForQualification(100000, 10, out reason))
                {
                    Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: death setup: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!router.CaptureEnhancedFrame(0f, 0f, false, -1, out reason))
                {
                    Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: death frame: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                deathObserved = source.PlayerDead || source.PlayerLife <= 0;
                generationBeforeRestart = source.Generation;
                if (!deathObserved)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: lethal owner qualification did not reach player-dead state" +
                        " life=" + source.PlayerLife +
                        " dead=" + source.PlayerDead);
                    EditorApplication.Exit(1);
                    return;
                }

                if (!router.ResetEnhancedOwnerSession(out reason))
                {
                    Debug.LogError("OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: restart reset: " + reason);
                    EditorApplication.Exit(1);
                    return;
                }

                generationAfterRestart = source.Generation;
                restartTick = source.Tick;
                if (generationAfterRestart <= generationBeforeRestart ||
                    restartTick != 0 ||
                    enhanced.Generation != generationAfterRestart ||
                    enhanced.LastTick != 0)
                {
                    Debug.LogError(
                        "OWNER UNITY ENHANCED SCENARIO SMOKE FAIL: restart did not produce a fresh tick-0 generation" +
                        " before=" + generationBeforeRestart +
                        " after=" + generationAfterRestart +
                        " tick=" + restartTick);
                    EditorApplication.Exit(1);
                    return;
                }

                Debug.Log(
                    "OWNER UNITY ENHANCED SCENARIO SMOKE PASS: idle_frames=" + IdleFrames +
                    " town_frames=" + TownFrames +
                    " combat_frames=" + CombatFrames +
                    " flight_frames=" + FlightFrames +
                    " town_events=" + townEvents +
                    " combat_events=" + combatEvents +
                    " combat_projectiles=" + combatProjectiles +
                    " combat_impacts=" + combatImpacts +
                    " max_flight_speed=" + maxFlightSpeed +
                    " flight_camera_distance=" + flightCameraDistance.ToString("F3") +
                    " death_observed=" + (deathObserved ? 1 : 0) +
                    " restart_generation=" + generationAfterRestart +
                    " restart_tick=" + restartTick +
                    " state_route_hash=" + SequenceHash(stateHashes).ToString("x16") +
                    " event_route_hash=" + SequenceHash(eventHeads).ToString("x16") +
                    " lighting=" + enhanced.NightLighting.HasDescriptor +
                    " reflection=" + enhanced.WaterPresentation.ReflectionEnabled);
                EditorApplication.Exit(0);
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(root);
            }
        }

        private static Vector2 SequenceInput(int frame)
        {
            if (frame < 8)
            {
                return Vector2.zero;
            }

            if (frame < 16)
            {
                return new Vector2(0.25f, 0f);
            }

            if (frame < 24)
            {
                return new Vector2(-0.25f, -0.15f);
            }

            return new Vector2(0f, 0.2f);
        }

        private static Vector2 ScenarioFlightInput(string route, int frame)
        {
            if (string.Equals(route, "town", StringComparison.Ordinal))
            {
                return new Vector2(0.18f, 0.06f);
            }

            if (string.Equals(route, "flight", StringComparison.Ordinal))
            {
                float horizontal = frame % 8 < 4 ? 0.35f : -0.35f;
                return new Vector2(horizontal, 0.25f);
            }

            return Vector2.zero;
        }

        private static Vector2 VisualCaptureInput(string route, int frame)
        {
            if (string.Equals(route, "town-approach", StringComparison.Ordinal))
            {
                return new Vector2(0.18f, 0.06f);
            }

            if (string.Equals(route, "town-overflight", StringComparison.Ordinal))
            {
                return new Vector2(frame % 8 < 4 ? 0.25f : -0.25f, 0.25f);
            }

            if (string.Equals(route, "mana-castle", StringComparison.Ordinal))
            {
                return new Vector2(0.2f, 0f);
            }

            if (string.Equals(route, "reflective-water", StringComparison.Ordinal))
            {
                return new Vector2(0f, 0.15f);
            }

            if (string.Equals(route, "fog-depth", StringComparison.Ordinal))
            {
                return new Vector2(0f, 0.1f);
            }

            if (string.Equals(route, "high-speed-flight", StringComparison.Ordinal))
            {
                return HighSpeedQualificationInput(frame);
            }

            return Vector2.zero;
        }

        private static Vector2 HighSpeedQualificationInput(int frame)
        {
            float horizontal = frame % 8 < 4 ? 0.35f : -0.35f;
            return new Vector2(horizontal, 0.25f);
        }

        private static Vector2 PerformanceQualificationInput(int frame)
        {
            int phase = frame % 32;
            if (phase < 8)
            {
                return Vector2.zero;
            }

            if (phase < 16)
            {
                return new Vector2(0.18f, 0.06f);
            }

            if (phase < 24)
            {
                return new Vector2(0.35f, 0.25f);
            }

            return new Vector2(-0.35f, 0.25f);
        }

        private static long PercentileNanoseconds(IReadOnlyList<long> values, double percentile)
        {
            if (values == null || values.Count == 0)
            {
                return 0;
            }

            var sorted = new List<long>(values);
            sorted.Sort();
            int index = (int)Math.Ceiling(percentile * sorted.Count) - 1;
            index = Math.Max(0, Math.Min(index, sorted.Count - 1));
            return sorted[index];
        }

        private static long ElapsedNanoseconds(long start)
        {
            long elapsedTicks = System.Diagnostics.Stopwatch.GetTimestamp() - start;
            return (long)(elapsedTicks * (1000000000.0 / System.Diagnostics.Stopwatch.Frequency));
        }

        private static long MaxValue(IReadOnlyList<long> values)
        {
            long maximum = 0;
            if (values == null)
            {
                return maximum;
            }

            for (int index = 0; index < values.Count; index++)
            {
                maximum = Math.Max(maximum, values[index]);
            }

            return maximum;
        }

        private static string Sha256Hex(byte[] bytes)
        {
            using (SHA256 sha256 = SHA256.Create())
            {
                byte[] digest = sha256.ComputeHash(bytes ?? Array.Empty<byte>());
                return BitConverter.ToString(digest).Replace("-", string.Empty).ToLowerInvariant();
            }
        }

        private static string Sha256File(string path)
        {
            return Sha256Hex(File.ReadAllBytes(path));
        }

        [Serializable]
        private sealed class ClassicHdOwnerCompletionReceipt
        {
            public int schema_version;
            public string mode;
            public string route;
            public string status;
            public string evidence_authority;
            public bool owner_payload_embedded;
            public bool owner_inputs_external;
            public string journal_schema;
            public string journal_status;
            public string journal_command_encoding;
            public int journal_command_count;
            public int journal_replayed_commands;
            public string journal_manifest_sha256;
            public string journal_commands_sha256;
            public int frame_count;
            public int upload_count;
            public int allocation_epoch;
            public ulong generation;
            public ulong initial_tick;
            public ulong final_tick;
            public ulong source_initial_tick;
            public ulong source_final_tick;
            public ulong authoritative_generation;
            public bool generation_stable;
            public bool exit_sequence_active_observed;
            public bool exit_sequence_active;
            public bool exit_sequence_complete;
            public byte exit_result_code;
            public byte tasks_completed;
            public short max_target_speed;
            public short max_actual_speed;
            public string index_route_hash;
            public string provenance_route_hash;
            public string reason;
        }

        [Serializable]
        private sealed class ClassicHdVisualCaptureReceipt
        {
            public int schema_version;
            public string mode;
            public string profile;
            public string capture_status;
            public string visual_review_status;
            public bool owner_payload_embedded;
            public bool owner_inputs_external;
            public string unity_editor;
            public string graphics_api;
            public string graphics_device;
            public string graphics_driver;
            public ClassicHdVisualCaptureEntry[] captures;
        }

        [Serializable]
        private sealed class ClassicHdVisualCaptureEntry
        {
            public string route_id;
            public string capture_name;
            public string file_name;
            public string file_sha256;
            public string resolved_png_hash;
            public int frame_count;
            public bool cast_observed;
            public int width;
            public int height;
            public int upload_count;
            public string source_generation;
            public string source_tick;
        }

        [Serializable]
        private sealed class UnityEnhancedVisualCaptureReceipt
        {
            public int schema_version;
            public string mode;
            public string profile;
            public string capture_status;
            public string visual_review_status;
            public bool owner_payload_embedded;
            public bool owner_inputs_external;
            public string unity_editor;
            public string graphics_api;
            public string graphics_device;
            public string graphics_driver;
            public UnityEnhancedVisualCaptureEntry[] captures;
        }

        [Serializable]
        private sealed class UnityEnhancedVisualCaptureEntry
        {
            public string route_id;
            public string capture_name;
            public string file_name;
            public string file_sha256;
            public string resolved_png_hash;
            public int frame_count;
            public bool cast_observed;
            public int width;
            public int height;
            public int active_entity_count;
            public int active_visual_count;
            public string source_generation;
            public string source_tick;
        }

        private static bool ReadGpuHashes(
            ClassicHdPresentation classic,
            out ulong rawHash,
            out ulong resolvedHash,
            out bool fenceValid,
            out bool fencePassed,
            out bool rawError,
            out bool resolvedError,
            out string reason)
        {
            rawHash = 0;
            resolvedHash = 0;
            fenceValid = false;
            fencePassed = false;
            rawError = false;
            resolvedError = false;
            reason = string.Empty;
            if (!SystemInfo.supportsAsyncGPUReadback)
            {
                reason = "Unity reports AsyncGPUReadback unsupported";
                return false;
            }
            if (!SystemInfo.supportsGraphicsFence)
            {
                reason = "Unity reports GraphicsFence unsupported";
                return false;
            }

            RenderTexture resolved = new RenderTexture(
                classic.IndexTexture.width,
                classic.IndexTexture.height,
                0,
                RenderTextureFormat.ARGB32,
                RenderTextureReadWrite.Linear);
            resolved.name = "ClassicHd_GPUFenceProbe_Resolved";
            try
            {
                resolved.Create();
                CommandBuffer command = new CommandBuffer
                {
                    name = "ClassicHd_GPUFenceProbe_Blit"
                };
                GraphicsFence fence;
                try
                {
                    command.SetRenderTarget(resolved);
                    command.ClearRenderTarget(true, true, Color.clear);
                    command.Blit(classic.IndexTexture, resolved, classic.IndexedMaterial);
                    fence = command.CreateGraphicsFence(
                        GraphicsFenceType.AsyncQueueSynchronisation,
                        SynchronisationStageFlags.PixelProcessing);
                    Graphics.ExecuteCommandBuffer(command);
                }
                finally
                {
                    command.Release();
                }

                AsyncGPUReadbackRequest rawRequest = AsyncGPUReadback.Request(
                    classic.IndexTexture,
                    0,
                    TextureFormat.R8);
                AsyncGPUReadbackRequest resolvedRequest = AsyncGPUReadback.Request(
                    resolved,
                    0,
                    TextureFormat.RGBA32);
                rawRequest.WaitForCompletion();
                resolvedRequest.WaitForCompletion();
                rawError = rawRequest.hasError;
                resolvedError = resolvedRequest.hasError;
                if (rawError || resolvedError)
                {
                    reason = "AsyncGPUReadback reported an error";
                    return false;
                }

                rawHash = HashReadback(rawRequest);
                resolvedHash = HashReadback(resolvedRequest);
                try
                {
                    fenceValid = true;
                    fencePassed = fence.passed;
                    if (!fencePassed)
                    {
                        reason = "graphics fence did not report passed";
                    }
                }
                catch (Exception exception)
                {
                    fenceValid = false;
                    fencePassed = false;
                    reason = "graphics fence status unavailable: " + exception.Message;
                }

                return true;
            }
            catch (Exception exception)
            {
                reason = "GPU fence/readback exception: " + exception.Message;
                return false;
            }
            finally
            {
                if (resolved != null)
                {
                    resolved.Release();
                    UnityEngine.Object.DestroyImmediate(resolved);
                }
            }
        }

        private static ulong HashReadback(AsyncGPUReadbackRequest request)
        {
            unchecked
            {
                ulong hash = 0xcbf29ce484222325UL;
                var data = request.GetData<byte>();
                for (int index = 0; index < data.Length; index++)
                {
                    hash ^= data[index];
                    hash *= 0x100000001b3UL;
                }

                return hash;
            }
        }

        private static long CameraDistance(ushort x, ushort y, ushort startX, ushort startY)
        {
            long dx = (long)x - startX;
            long dy = (long)y - startY;
            return dx * dx + dy * dy;
        }

        private static ulong SequenceHash(IReadOnlyList<ulong> values)
        {
            unchecked
            {
                ulong hash = 0xcbf29ce484222325UL;
                for (int index = 0; index < values.Count; index++)
                {
                    hash ^= values[index];
                    hash *= 0x100000001b3UL;
                }

                return hash;
            }
        }

        private static string CommandLineValue(string name)
        {
            string[] arguments = Environment.GetCommandLineArgs();
            for (int index = 0; index < arguments.Length; index++)
            {
                if (string.Equals(arguments[index], name, StringComparison.OrdinalIgnoreCase))
                {
                    return index + 1 < arguments.Length ? arguments[index + 1] : string.Empty;
                }

                string prefix = name + "=";
                if (arguments[index].StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    return arguments[index].Substring(prefix.Length);
                }
            }

            return string.Empty;
        }

        private static void EnsureFolder(string path)
        {
            if (AssetDatabase.IsValidFolder(path))
            {
                return;
            }

            string parent = System.IO.Path.GetDirectoryName(path).Replace('\\', '/');
            string name = System.IO.Path.GetFileName(path);
            AssetDatabase.CreateFolder(parent, name);
        }
    }
}
#endif
