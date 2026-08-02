#if UNITY_EDITOR
using System;
using System.Collections.Generic;
using System.Reflection;
using UnityEditor;
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
