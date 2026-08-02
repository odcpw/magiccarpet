using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.XR;
using UnityEngine.XR.Management;
using Unity.XR.CoreUtils;

namespace MagicCarpet.PresentationLab
{
    public enum XrSmokeProfile
    {
        Disabled = 0,
        PcOpenXr = 1,
    }

    public static class XrSmokeProfileParser
    {
        public static XrSmokeProfile FromCommandLine(string[] arguments, XrSmokeProfile fallback)
        {
            string value = CommandLineValue(arguments, "-mc-xr-profile");
            if (string.IsNullOrWhiteSpace(value))
            {
                return fallback;
            }

            if (string.Equals(value, "off", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(value, "disabled", StringComparison.OrdinalIgnoreCase))
            {
                return XrSmokeProfile.Disabled;
            }

            if (string.Equals(value, "pc-openxr", StringComparison.OrdinalIgnoreCase))
            {
                return XrSmokeProfile.PcOpenXr;
            }

            return XrSmokeProfile.Disabled;
        }

        private static string CommandLineValue(string[] arguments, string name)
        {
            for (int index = 0; index < (arguments == null ? 0 : arguments.Length); index++)
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
    }

    /// <summary>
    /// Owner-free PC OpenXR bootstrap. It is intentionally independent from
    /// the Rust session and creates only a diagnostic XR Origin shell.
    /// </summary>
    public sealed class XrSmokeBootstrap : MonoBehaviour
    {
        [SerializeField] private XrSmokeProfile defaultProfile = XrSmokeProfile.Disabled;

        private bool loaderStarted;
        private bool smokeOnce;
        private bool shutdownScheduled;
        private GameObject syntheticOrigin;

        public XrSmokeProfile ActiveProfile { get; private set; }
        public bool LoaderStarted { get { return loaderStarted; } }
        public string LastStatus { get; private set; }

        private void Awake()
        {
            string[] arguments = Environment.GetCommandLineArgs();
            smokeOnce = HasFlag(arguments, "-mc-xr-smoke-once");
            ActiveProfile = XrSmokeProfileParser.FromCommandLine(
                arguments,
                defaultProfile);

            if (ActiveProfile == XrSmokeProfile.Disabled)
            {
                LastStatus = "XR DISABLED: flat presentation has no XR loader or XR Origin";
                Debug.Log("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
                FinishSmoke(0);
                return;
            }

            StartPcOpenXr();
        }

        private void OnDestroy()
        {
            if (loaderStarted && XRGeneralSettings.Instance != null && XRGeneralSettings.Instance.Manager != null)
            {
                XRManagerSettings manager = XRGeneralSettings.Instance.Manager;
                manager.StopSubsystems();
                manager.DeinitializeLoader();
                loaderStarted = false;
                Debug.Log("[MC2 XR SMOKE] XR loader shutdown complete; owner-data=none; simulation=none");
            }

            if (syntheticOrigin != null)
            {
                Destroy(syntheticOrigin);
                syntheticOrigin = null;
            }
        }

        private void StartPcOpenXr()
        {
            XRGeneralSettings general = XRGeneralSettings.Instance;
            if (general == null || general.Manager == null)
            {
                Fail("PC OpenXR profile requested but XR Plug-in Management is unavailable");
                FinishSmoke(2);
                return;
            }

            XRManagerSettings manager = general.Manager;
            try
            {
                manager.InitializeLoaderSync();
            }
            catch (Exception exception)
            {
                Fail("PC OpenXR loader initialization threw: " + exception.GetType().Name);
                FinishSmoke(2);
                return;
            }

            if (manager.activeLoader == null)
            {
                Fail("PC OpenXR runtime or supported loader is unavailable");
                FinishSmoke(2);
                return;
            }

            try
            {
                manager.StartSubsystems();
            }
            catch (Exception exception)
            {
                Fail("PC OpenXR subsystem startup threw: " + exception.GetType().Name);
                FinishSmoke(2);
                return;
            }

            loaderStarted = true;
            CreateSyntheticOrigin();
            bool controllersAvailable = ReportRuntimeIdentity();
            if (smokeOnce && !controllersAvailable)
            {
                FinishSmoke(3);
                return;
            }

            ScheduleSmokeShutdown();
        }

        private void CreateSyntheticOrigin()
        {
            syntheticOrigin = new GameObject("XR Origin");
            syntheticOrigin.SetActive(false);
            GameObject cameraOffset = new GameObject("Camera Offset");
            cameraOffset.transform.SetParent(syntheticOrigin.transform, false);

            GameObject head = new GameObject("Tracked Head");
            head.transform.SetParent(cameraOffset.transform, false);
            Camera camera = head.AddComponent<Camera>();
            camera.tag = "MainCamera";

            GameObject leftController = new GameObject("Left Controller");
            leftController.transform.SetParent(syntheticOrigin.transform, false);
            GameObject rightController = new GameObject("Right Controller");
            rightController.transform.SetParent(syntheticOrigin.transform, false);

            XROrigin origin = syntheticOrigin.AddComponent<XROrigin>();
            origin.Origin = syntheticOrigin;
            origin.CameraFloorOffsetObject = cameraOffset;
            origin.Camera = camera;
            origin.RequestedTrackingOriginMode = XROrigin.TrackingOriginMode.Floor;
            syntheticOrigin.SetActive(true);
        }

        private bool ReportRuntimeIdentity()
        {
            var devices = new List<InputDevice>();
            InputDevices.GetDevices(devices);
            var displays = new List<XRDisplaySubsystem>();
            SubsystemManager.GetSubsystems(displays);

            int headsetCount = 0;
            int leftControllerCount = 0;
            int rightControllerCount = 0;
            var identities = new List<string>();
            for (int index = 0; index < devices.Count; index++)
            {
                InputDevice device = devices[index];
                InputDeviceCharacteristics characteristics = device.characteristics;
                if ((characteristics & InputDeviceCharacteristics.HeadMounted) != 0)
                {
                    headsetCount++;
                }

                bool isController = (characteristics & InputDeviceCharacteristics.Controller) != 0;
                if (isController && (characteristics & InputDeviceCharacteristics.Left) != 0)
                {
                    leftControllerCount++;
                }

                if (isController && (characteristics & InputDeviceCharacteristics.Right) != 0)
                {
                    rightControllerCount++;
                }

                string identity = string.IsNullOrWhiteSpace(device.name) ? "unnamed" : device.name;
                if (!string.IsNullOrWhiteSpace(device.manufacturer))
                {
                    identity += "/" + device.manufacturer;
                }

                identities.Add(identity + "[" + characteristics + "]");
            }

            string identitySummary = identities.Count == 0 ? "none" : string.Join(",", identities.ToArray());
            LastStatus = "PC OPENXR ACTIVE: loader=" + XRGeneralSettings.Instance.Manager.activeLoader.name +
                         " / display-subsystems=" + displays.Count.ToString() +
                         " / input-devices=" + devices.Count.ToString() +
                         " / headsets=" + headsetCount.ToString() +
                         " / left-controllers=" + leftControllerCount.ToString() +
                         " / right-controllers=" + rightControllerCount.ToString() +
                         " / identities=" + identitySummary;
            Debug.Log("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
            if (leftControllerCount == 0 || rightControllerCount == 0)
            {
                Debug.LogError("[MC2 XR SMOKE] XR CONTROLLERS UNAVAILABLE: expected left and right controller identities");
                return false;
            }

            return true;
        }

        private void Fail(string reason)
        {
            ActiveProfile = XrSmokeProfile.Disabled;
            LastStatus = "XR UNAVAILABLE: " + reason;
            Debug.LogError("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
        }

        private void ScheduleSmokeShutdown()
        {
            if (!smokeOnce || shutdownScheduled)
            {
                return;
            }

            shutdownScheduled = true;
            Invoke(nameof(CompleteSmoke), 1.0f);
        }

        private void CompleteSmoke()
        {
            Debug.Log("[MC2 XR SMOKE] shutdown requested; loader teardown follows OnDestroy");
            Application.Quit(0);
        }

        private void FinishSmoke(int exitCode)
        {
            if (!smokeOnce)
            {
                return;
            }

            Debug.Log("[MC2 XR SMOKE] smoke completed with exit-code=" + exitCode.ToString());
            Application.Quit(exitCode);
        }

        private static bool HasFlag(string[] arguments, string name)
        {
            for (int index = 0; index < (arguments == null ? 0 : arguments.Length); index++)
            {
                if (string.Equals(arguments[index], name, StringComparison.OrdinalIgnoreCase))
                {
                    return true;
                }
            }

            return false;
        }
    }
}
