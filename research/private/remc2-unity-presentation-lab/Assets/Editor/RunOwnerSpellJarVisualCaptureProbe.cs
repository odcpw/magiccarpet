#if UNITY_EDITOR
using System;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.PresentationLab.Editor
{
    /// <summary>
    /// Captures one real owner-driven SpellJar frame for human visual review.
    /// The probe never supplies a fixture snapshot or embeds owner payload.
    /// </summary>
    public static class RunOwnerSpellJarVisualCaptureProbe
    {
        private const int CaptureWidth = 1280;
        private const int CaptureHeight = 720;
        private const int FrameLimit = 512;
        private const float InnerViewportMinimum = 0.12f;
        private const float InnerViewportMaximum = 0.88f;

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner SpellJar Visual Capture Probe")]
        public static void Run()
        {
            RequirePresentationMode();
            string packagePath = RequiredCommandLineValue("-mc-package");
            string closurePath = RequiredCommandLineValue("-mc-closure");
            string assetCachePath = RequiredCommandLineValue("-mc-asset-cache");
            string captureDirectory = Path.GetFullPath(
                RequiredCommandLineValue("-mc-capture-dir"));
            Directory.CreateDirectory(captureDirectory);

            string outputPath = Path.Combine(
                captureDirectory,
                "l1-night-owner-spell-jar.png");
            string receiptPath = Path.Combine(
                captureDirectory,
                "unity-enhanced-owner-spell-jar-capture-receipt.json");
            RefuseExistingOutput(outputPath, receiptPath);

            GameObject routerObject = new GameObject(
                "Owner Unity Enhanced SpellJar Visual Capture Probe Router");
            PresentationModeRouter router = routerObject.AddComponent<PresentationModeRouter>();
            UnityEnhancedWorldRoot enhanced = null;
            UnityEnhancedOwnerSessionSource source = null;
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(routerObject);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null)
                {
                    FailAndExit(
                        "owner route did not open" +
                        " status=" + router.StatusLine +
                        " owner_validation=" +
                        (router.OwnerValidation == null
                            ? "null"
                            : router.OwnerValidation.Reason));
                    return;
                }

                enhanced = router.EnhancedRoot;
                source = router.EnhancedOwnerSession;
                Camera presentationCamera = enhanced.CameraRig == null
                    ? null
                    : enhanced.CameraRig.PresentationCamera;
                if (presentationCamera == null)
                {
                    FailAndExit("owner route opened without an Enhanced presentation camera");
                    return;
                }

                int spellJarEntitiesObserved = 0;
                bool captured = false;
                int captureFrame = -1;
                int selectedSourceEntityIndex = -1;
                ulong selectedStableIdentity = 0;
                ulong selectedGeneration = 0;
                ulong selectedTick = 0;
                Vector3 selectedViewport = Vector3.zero;
                byte[] pngBytes = null;
                string reason = string.Empty;

                for (int frame = 0; frame < FrameLimit; frame++)
                {
                    // These are modest owner-facing flight inputs. The camera
                    // values oscillate through the existing native camera ABI;
                    // no world position or camera transform is supplied here.
                    float horizontalInput = 0.14f * Mathf.Sin(frame * 0.071f);
                    float verticalInput = 0.09f * Mathf.Cos(frame * 0.053f);
                    float lookYawInput = 0.24f * Mathf.Sin(frame * 0.047f);
                    float lookPitchInput = 0.12f * Mathf.Sin(frame * 0.083f);
                    UnityEnhancedWorldSnapshot snapshot;
                    if (!source.Advance(
                            horizontalInput,
                            verticalInput,
                            lookYawInput,
                            lookPitchInput,
                            false,
                            -1,
                            out snapshot,
                            out reason))
                    {
                        FailAndExit("owner SpellJar frame " + frame + ": " + reason);
                        return;
                    }

                    if (!enhanced.ApplySnapshot(snapshot))
                    {
                        FailAndExit(
                            "Enhanced root rejected owner SpellJar frame " + frame +
                            (string.IsNullOrEmpty(enhanced.LastRejectReason)
                                ? string.Empty
                                : ": " + enhanced.LastRejectReason));
                        return;
                    }

                    enhanced.InterpolatePresentation(1f);
                    UnityEnhancedEntitySnapshot[] entities = snapshot.Entities;
                    if (entities == null)
                    {
                        continue;
                    }

                    for (int index = 0; index < entities.Length; index++)
                    {
                        UnityEnhancedEntitySnapshot entity = entities[index];
                        if (entity == null ||
                            !entity.Visible ||
                            entity.PresentationFamily !=
                                (int)UnityEnhancedPresentationFamily.SpellJar)
                        {
                            continue;
                        }

                        spellJarEntitiesObserved++;
                        Vector3 worldCenter;
                        if (!TryGetPresentationCenter(
                                enhanced,
                                entity.StableIdentity,
                                out worldCenter))
                        {
                            continue;
                        }
                        Vector3 viewport = presentationCamera.WorldToViewportPoint(worldCenter);
                        bool inFront = viewport.z > presentationCamera.nearClipPlane;
                        bool inInnerViewport = inFront &&
                            viewport.x >= InnerViewportMinimum &&
                            viewport.x <= InnerViewportMaximum &&
                            viewport.y >= InnerViewportMinimum &&
                            viewport.y <= InnerViewportMaximum;
                        if (!inInnerViewport)
                        {
                            continue;
                        }

                        selectedSourceEntityIndex = entity.SourceEntityIndex;
                        selectedStableIdentity = entity.StableIdentity;
                        selectedGeneration = snapshot.Generation;
                        selectedTick = snapshot.Tick;
                        selectedViewport = viewport;
                        captureFrame = frame;
                        captured = true;
                        Debug.Log(
                            "OWNER SPELLJAR VISUAL CAPTURE CASE PASS frame=" + frame +
                            " source_entity_index=" + selectedSourceEntityIndex +
                            " stable_identity=" + selectedStableIdentity +
                            " generation=" + selectedGeneration +
                            " tick=" + selectedTick +
                            " viewport=" + selectedViewport +
                            " spelljar_entities_observed=" + spellJarEntitiesObserved);
                        break;
                    }

                    if (captured)
                    {
                        break;
                    }
                }

                if (!captured)
                {
                    FailAndExit(
                        "no live SpellJar center entered the inner viewport after " +
                        FrameLimit + " frames; spelljar_entities_observed=" +
                        spellJarEntitiesObserved);
                    return;
                }

                ulong resolvedHash;
                if (!enhanced.CapturePresentationFramePng(
                        CaptureWidth,
                        CaptureHeight,
                        out pngBytes,
                        out resolvedHash,
                        out reason))
                {
                    FailAndExit(
                        "resolved owner SpellJar capture failed at frame " +
                        captureFrame + ": " + reason);
                    return;
                }

                RefuseExistingOutput(outputPath, receiptPath);
                string pngSha256 = Sha256Hex(pngBytes);
                string editorVersion = Application.unityVersion;
                string graphicsApi = SystemInfo.graphicsDeviceType.ToString();
                var receipt = new OwnerSpellJarVisualCaptureReceipt
                {
                    schema_version = 1,
                    mode = "unity-enhanced",
                    capture_name = "l1-night-owner-spell-jar",
                    capture_status = "captured-for-human-review",
                    visual_review_status = "not-reviewed",
                    owner_inputs_external = true,
                    synthetic_input_payload = false,
                    unity_editor = editorVersion,
                    graphics_api = graphicsApi,
                    editor_sha256 = Sha256Hex(editorVersion),
                    api_sha256 = Sha256Hex(graphicsApi),
                    png_file = Path.GetFileName(outputPath),
                    png_sha256 = pngSha256,
                    resolved_png_hash = resolvedHash.ToString("x16"),
                    frames_scanned = captureFrame + 1,
                    spelljar_entities_observed = spellJarEntitiesObserved,
                    selected_frame = captureFrame,
                    selected_source_entity_index = selectedSourceEntityIndex,
                    selected_stable_identity = selectedStableIdentity.ToString(),
                    selected_generation = selectedGeneration.ToString(),
                    selected_tick = selectedTick.ToString(),
                    selected_viewport_x = selectedViewport.x,
                    selected_viewport_y = selectedViewport.y,
                    selected_viewport_z = selectedViewport.z,
                    no_synthetic_input_payload = "none"
                };

                File.WriteAllBytes(outputPath, pngBytes);
                File.WriteAllText(
                    receiptPath,
                    JsonUtility.ToJson(receipt, true) + "\n");
                Debug.Log(
                    "OWNER SPELLJAR VISUAL CAPTURE PROBE PASS frame=" + captureFrame +
                    " generation=" + selectedGeneration +
                    " tick=" + selectedTick +
                    " source_entity_index=" + selectedSourceEntityIndex +
                    " viewport=" + selectedViewport +
                    " frames_scanned=" + (captureFrame + 1) +
                    " spelljar_entities_observed=" + spellJarEntitiesObserved +
                    " output=" + outputPath +
                    " png_sha256=" + pngSha256 +
                    " visual_review_status=not-reviewed" +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            catch (Exception exception)
            {
                FailAndExit("owner SpellJar visual capture exception: " + exception);
            }
            finally
            {
                if (source != null)
                {
                    source.Dispose();
                }

                if (enhanced != null)
                {
                    GameObject enhancedObject = enhanced.gameObject;
                    enhanced.Close();
                    if (enhancedObject != routerObject)
                    {
                        UnityEngine.Object.DestroyImmediate(enhancedObject);
                    }
                }

                if (routerObject != null)
                {
                    UnityEngine.Object.DestroyImmediate(routerObject);
                }
            }
        }

        private static void RequirePresentationMode()
        {
            string mode = RequiredCommandLineValue("-mc-presentation-mode");
            if (!string.Equals(mode, "unity-enhanced", StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidOperationException(
                    "owner SpellJar visual capture requires -mc-presentation-mode unity-enhanced");
            }
        }

        private static string RequiredCommandLineValue(string name)
        {
            string value = CommandLineValue(name);
            if (string.IsNullOrWhiteSpace(value))
            {
                throw new InvalidOperationException(
                    "owner SpellJar visual capture requires " + name);
            }

            return value;
        }

        private static void RefuseExistingOutput(string outputPath, string receiptPath)
        {
            if (File.Exists(outputPath) || File.Exists(receiptPath))
            {
                throw new InvalidOperationException(
                    "refusing to overwrite an existing owner SpellJar capture in " +
                    Path.GetDirectoryName(outputPath));
            }
        }

        private static bool TryGetPresentationCenter(
            UnityEnhancedWorldRoot enhanced,
            ulong stableIdentity,
            out Vector3 center)
        {
            center = Vector3.zero;
            Transform entity = enhanced.transform.Find(
                "EnhancedWorldObjects/EnhancedEntity_" + stableIdentity.ToString("x16"));
            if (entity == null || !entity.gameObject.activeInHierarchy)
            {
                return false;
            }

            Transform visual = entity.Find("EnhancedActorVisual");
            if (visual == null || !visual.gameObject.activeInHierarchy)
            {
                return false;
            }

            MeshFilter meshFilter = visual.GetComponent<MeshFilter>();
            MeshRenderer meshRenderer = visual.GetComponent<MeshRenderer>();
            if (meshFilter == null ||
                meshFilter.sharedMesh == null ||
                meshFilter.sharedMesh.vertexCount == 0 ||
                meshRenderer == null ||
                !meshRenderer.enabled ||
                !meshRenderer.gameObject.activeInHierarchy)
            {
                return false;
            }

            center = meshRenderer.bounds.center;
            return true;
        }

        private static void FailAndExit(string message)
        {
            Debug.LogError("OWNER SPELLJAR VISUAL CAPTURE PROBE FAIL: " + message);
            EditorApplication.Exit(1);
        }

        private static string Sha256Hex(string value)
        {
            return Sha256Hex(System.Text.Encoding.UTF8.GetBytes(value ?? string.Empty));
        }

        private static string Sha256Hex(byte[] bytes)
        {
            using (SHA256 sha256 = SHA256.Create())
            {
                byte[] digest = sha256.ComputeHash(bytes ?? Array.Empty<byte>());
                return BitConverter.ToString(digest).Replace("-", string.Empty).ToLowerInvariant();
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

        [Serializable]
        private sealed class OwnerSpellJarVisualCaptureReceipt
        {
            public int schema_version;
            public string mode;
            public string capture_name;
            public string capture_status;
            public string visual_review_status;
            public bool owner_inputs_external;
            public bool synthetic_input_payload;
            public string unity_editor;
            public string graphics_api;
            public string editor_sha256;
            public string api_sha256;
            public string png_file;
            public string png_sha256;
            public string resolved_png_hash;
            public int frames_scanned;
            public int spelljar_entities_observed;
            public int selected_frame;
            public int selected_source_entity_index;
            public string selected_stable_identity;
            public string selected_generation;
            public string selected_tick;
            public float selected_viewport_x;
            public float selected_viewport_y;
            public float selected_viewport_z;
            public string no_synthetic_input_payload;
        }
    }
}
#endif
