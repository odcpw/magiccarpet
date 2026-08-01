using System;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum PresentationMode
    {
        ClassicHd = 0,
        UnityEnhanced = 1,
    }

    public readonly struct PresentationLabBootResult
    {
        public PresentationLabBootResult(
            bool ready,
            PresentationMode mode,
            bool synthetic,
            string message)
        {
            Ready = ready;
            Mode = mode;
            Synthetic = synthetic;
            Message = message;
        }

        public bool Ready { get; }
        public PresentationMode Mode { get; }
        public bool Synthetic { get; }
        public string Message { get; }
    }

    [DisallowMultipleComponent]
    public sealed class PresentationLabBootstrap : MonoBehaviour
    {
        [SerializeField] private PresentationMode defaultMode = PresentationMode.UnityEnhanced;
        [SerializeField] private bool requireOwnerDataForGameplay;

        private PresentationLabSession session;

        public static PresentationLabBootResult LastResult { get; private set; }

        private void Awake()
        {
            LastResult = Evaluate(
                Environment.GetCommandLineArgs(),
                defaultMode,
                requireOwnerDataForGameplay,
                Directory.Exists);

            if (!LastResult.Ready)
            {
                Debug.LogError("[MC2 presentation lab] " + LastResult.Message);
                enabled = false;
                return;
            }

            session = new PresentationLabSession(LastResult.Mode, LastResult.Synthetic);
            session.Start();
            Debug.Log("[MC2 presentation lab] " + LastResult.Message);
        }

        private void OnApplicationQuit()
        {
            session?.Dispose();
            session = null;
        }

        public static PresentationLabBootResult Evaluate(
            string[] args,
            PresentationMode fallback,
            bool requireOwner,
            Func<string, bool> directoryExists)
        {
            var mode = fallback;
            var ownerRoot = string.Empty;
            var explicitMode = false;
            var requireOwnerFromArgs = false;

            foreach (var arg in args ?? Array.Empty<string>())
            {
                if (arg.StartsWith("--mc-mode=", StringComparison.OrdinalIgnoreCase))
                {
                    explicitMode = true;
                    var value = arg.Substring("--mc-mode=".Length);
                    if (string.Equals(value, "classic-hd", StringComparison.OrdinalIgnoreCase))
                    {
                        mode = PresentationMode.ClassicHd;
                    }
                    else if (string.Equals(value, "enhanced", StringComparison.OrdinalIgnoreCase) ||
                             string.Equals(value, "unity-enhanced", StringComparison.OrdinalIgnoreCase))
                    {
                        mode = PresentationMode.UnityEnhanced;
                    }
                    else
                    {
                        return new PresentationLabBootResult(
                            false,
                            fallback,
                            false,
                            "invalid --mc-mode; expected classic-hd or enhanced");
                    }
                }
                else if (arg.StartsWith("--mc-owner-root=", StringComparison.OrdinalIgnoreCase))
                {
                    ownerRoot = arg.Substring("--mc-owner-root=".Length);
                }
                else if (string.Equals(arg, "--mc-require-owner-data", StringComparison.OrdinalIgnoreCase))
                {
                    requireOwnerFromArgs = true;
                }
            }

            var ownerRequired = requireOwner || requireOwnerFromArgs;
            var ownerPresent = !string.IsNullOrWhiteSpace(ownerRoot) && directoryExists(ownerRoot);
            if (ownerRequired && !ownerPresent)
            {
                return new PresentationLabBootResult(
                    false,
                    mode,
                    false,
                    "owner data is required but --mc-owner-root is absent or not a directory");
            }

            var synthetic = !ownerPresent;
            var modeLabel = mode == PresentationMode.ClassicHd ? "classic-hd" : "unity-enhanced";
            var sourceLabel = synthetic ? "synthetic/no-owner" : "external owner root";
            var selectionLabel = explicitMode ? "explicit" : "default";
            return new PresentationLabBootResult(
                true,
                mode,
                synthetic,
                $"booted {modeLabel} ({selectionLabel}) with {sourceLabel}; no gameplay authority in Unity");
        }
    }

    internal sealed class PresentationLabSession : IDisposable
    {
        private readonly PresentationMode mode;
        private readonly bool synthetic;
        private bool started;

        public PresentationLabSession(PresentationMode mode, bool synthetic)
        {
            this.mode = mode;
            this.synthetic = synthetic;
        }

        public void Start()
        {
            if (started)
            {
                return;
            }

            started = true;
            Debug.Log($"[MC2 presentation lab] session started mode={mode} source={(synthetic ? "synthetic" : "external")}");
        }

        public void Dispose()
        {
            if (!started)
            {
                return;
            }

            started = false;
            Debug.Log("[MC2 presentation lab] session disposed");
        }
    }
}
