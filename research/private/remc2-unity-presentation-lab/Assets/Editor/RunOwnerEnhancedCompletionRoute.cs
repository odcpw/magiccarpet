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
    /// Replays the external PlaythroughRecorder command stream through Unity
    /// Enhanced. It is the parity/completion gate: a short or incomplete
    /// journal produces an honest open receipt, never an invented win state.
    /// </summary>
    public static class RunOwnerEnhancedCompletionRoute
    {
        private const int MinimumLevel1CommandCount = 1000;

        [MenuItem("Magic Carpet/Presentation Lab/Run Owner Unity Enhanced Completion Route")]
        public static void RunOwnerUnityEnhancedCompletionRoute()
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
                    "owner Unity Enhanced completion route requires -mc-package, -mc-closure, " +
                    "-mc-asset-cache, -mc-command-journal, and -mc-completion-receipt");
            }

            receiptPath = Path.GetFullPath(receiptPath);
            if (File.Exists(receiptPath))
            {
                throw new InvalidOperationException(
                    "refusing to overwrite an existing Unity Enhanced completion receipt: " + receiptPath);
            }

            GameObject root = new GameObject("Owner Unity Enhanced Completion Route Router");
            PresentationModeRouter router = root.AddComponent<PresentationModeRouter>();
            MethodInfo awake = typeof(PresentationModeRouter).GetMethod(
                "Awake",
                BindingFlags.Instance | BindingFlags.NonPublic);
            if (awake == null)
            {
                UnityEngine.Object.DestroyImmediate(root);
                throw new InvalidOperationException("presentation router Awake method is missing");
            }

            ClassicHdOwnerCommandJournal journal = null;
            UnityEnhancedOwnerSessionSource source = null;
            UnityEnhancedWorldRoot enhanced = null;
            try
            {
                awake.Invoke(router, null);
                if (!router.SessionIsOpen ||
                    router.EnhancedRoot == null ||
                    router.EnhancedOwnerSession == null)
                {
                    throw new InvalidOperationException(
                        "owner Unity Enhanced completion route did not open: " + router.StatusLine);
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

                if (journal.CommandCount < MinimumLevel1CommandCount)
                {
                    throw new InvalidOperationException(
                        "owner Unity Enhanced completion journal is too short for a Level 1 route: " +
                        journal.CommandCount);
                }

                source = router.EnhancedOwnerSession;
                enhanced = router.EnhancedRoot;
                ulong initialGeneration = source.Generation;
                ulong routeHash = 0xcbf29ce484222325UL;
                bool exitSequenceActiveObserved = false;
                short maxTargetSpeed = short.MinValue;
                short maxActualSpeed = short.MinValue;

                for (int commandIndex = 0; commandIndex < journal.CommandCount; commandIndex++)
                {
                    ClassicHdOwnerCommand command;
                    if (!journal.ReadNext(out command, out reason))
                    {
                        throw new InvalidOperationException(reason);
                    }

                    if (!router.CaptureEnhancedOwnerCommand(
                            command.Opcode,
                            command.Argument1,
                            command.Argument2,
                            command.Roll,
                            command.Pitch,
                            command.Flags,
                            command.RequestedSpell,
                            out reason))
                    {
                        throw new InvalidOperationException(
                            "owner Unity Enhanced completion command " + commandIndex + " failed: " + reason);
                    }

                    ulong expectedTick = checked((ulong)commandIndex + 1UL);
                    if (source.Generation != initialGeneration || source.Tick != expectedTick)
                    {
                        throw new InvalidOperationException(
                            "owner Unity Enhanced completion route changed generation or clock at command " +
                            commandIndex +
                            ": generation=" + source.Generation +
                            " expected_generation=" + initialGeneration +
                            " tick=" + source.Tick +
                            " expected_tick=" + expectedTick);
                    }

                    routeHash = Mix(routeHash, source.LastStateHash);
                    routeHash = Mix(routeHash, source.LastWorldHash);
                    exitSequenceActiveObserved |= source.ExitSequenceActive;
                    maxTargetSpeed = Math.Max(maxTargetSpeed, source.TargetSpeed);
                    maxActualSpeed = Math.Max(maxActualSpeed, (short)Math.Abs((int)source.ActualSpeed));
                    if ((commandIndex % 1024) == 0 || commandIndex + 1 == journal.CommandCount)
                    {
                        Debug.Log(
                            "OWNER UNITY ENHANCED COMPLETION ROUTE PROGRESS: command=" +
                            (commandIndex + 1) + "/" + journal.CommandCount +
                            " tick=" + source.Tick +
                            " stage_tasks=" + source.TasksCompleted +
                            " exit_active=" + (source.ExitSequenceActive ? 1 : 0) +
                            " exit_complete=" + (source.ExitSequenceComplete ? 1 : 0));
                    }
                }

                if (journal.ReadCount != journal.CommandCount)
                {
                    throw new InvalidOperationException(
                        "owner Unity Enhanced completion route did not consume the full command journal");
                }

                bool terminal = exitSequenceActiveObserved &&
                                source.ExitSequenceComplete &&
                                source.ExitResultCode == 10 &&
                                source.TasksCompleted > 0;
                string status = terminal ? "pass" : "open";
                string routeReason = terminal
                    ? "owner journal reached the authored exit result"
                    : "owner journal replay completed without the qualified exit result";
                var receipt = new UnityEnhancedCompletionReceipt
                {
                    schema_version = 1,
                    mode = "unity-enhanced",
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
                    frame_count = journal.ReadCount,
                    generation = source.Generation,
                    initial_tick = 0,
                    final_tick = source.Tick,
                    exit_sequence_active_observed = exitSequenceActiveObserved,
                    exit_sequence_active = source.ExitSequenceActive,
                    exit_sequence_complete = source.ExitSequenceComplete,
                    exit_result_code = source.ExitResultCode,
                    tasks_completed = source.TasksCompleted,
                    max_target_speed = maxTargetSpeed,
                    max_actual_speed = maxActualSpeed,
                    final_state_hash = source.LastStateHash.ToString("x16"),
                    final_world_hash = source.LastWorldHash.ToString("x16"),
                    route_hash = routeHash.ToString("x16"),
                    reason = routeReason
                };
                File.WriteAllText(receiptPath, JsonUtility.ToJson(receipt, true) + "\n");

                Debug.Log(
                    "OWNER UNITY ENHANCED COMPLETION ROUTE " + status.ToUpperInvariant() +
                    ": commands=" + journal.ReadCount +
                    " tick=" + source.Tick +
                    " tasks=" + source.TasksCompleted +
                    " exit_complete=" + (source.ExitSequenceComplete ? 1 : 0) +
                    " result=" + source.ExitResultCode +
                    " receipt=" + receiptPath +
                    " owner_inputs_external=true");
                EditorApplication.Exit(0);
            }
            catch (Exception exception)
            {
                Debug.LogError("OWNER UNITY ENHANCED COMPLETION ROUTE FAIL: " + exception.Message);
                EditorApplication.Exit(1);
            }
            finally
            {
                if (journal != null)
                {
                    journal.Dispose();
                }
                if (source != null)
                {
                    source.Dispose();
                }
                if (enhanced != null)
                {
                    enhanced.Close();
                }
                if (root != null)
                {
                    UnityEngine.Object.DestroyImmediate(root);
                }
            }
        }

        private static ulong Mix(ulong hash, ulong value)
        {
            unchecked
            {
                hash ^= value;
                return hash * 0x100000001b3UL;
            }
        }

        private static string Sha256File(string path)
        {
            using (SHA256 sha256 = SHA256.Create())
            {
                return BitConverter.ToString(sha256.ComputeHash(File.ReadAllBytes(path)))
                    .Replace("-", string.Empty)
                    .ToLowerInvariant();
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
        private sealed class UnityEnhancedCompletionReceipt
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
            public ulong generation;
            public ulong initial_tick;
            public ulong final_tick;
            public bool exit_sequence_active_observed;
            public bool exit_sequence_active;
            public bool exit_sequence_complete;
            public byte exit_result_code;
            public byte tasks_completed;
            public short max_target_speed;
            public short max_actual_speed;
            public string final_state_hash;
            public string final_world_hash;
            public string route_hash;
            public string reason;
        }
    }
}
#endif
