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
        private GameObject syntheticOrigin;

        public XrSmokeProfile ActiveProfile { get; private set; }
        public bool LoaderStarted { get { return loaderStarted; } }
        public string LastStatus { get; private set; }

        private void Awake()
        {
            ActiveProfile = XrSmokeProfileParser.FromCommandLine(
                Environment.GetCommandLineArgs(),
                defaultProfile);

            if (ActiveProfile == XrSmokeProfile.Disabled)
            {
                LastStatus = "XR DISABLED: flat presentation has no XR loader or XR Origin";
                Debug.Log("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
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
                return;
            }

            if (manager.activeLoader == null)
            {
                Fail("PC OpenXR runtime or supported loader is unavailable");
                return;
            }

            manager.StartSubsystems();
            loaderStarted = true;
            CreateSyntheticOrigin();
            ReportRuntimeIdentity();
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

        private void ReportRuntimeIdentity()
        {
            var devices = new List<InputDevice>();
            InputDevices.GetDevices(devices);
            var displays = new List<XRDisplaySubsystem>();
            SubsystemManager.GetSubsystems(displays);
            LastStatus = "PC OPENXR ACTIVE: loader=" + XRGeneralSettings.Instance.Manager.activeLoader.name +
                         " / display-subsystems=" + displays.Count.ToString() +
                         " / input-devices=" + devices.Count.ToString();
            Debug.Log("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
        }

        private void Fail(string reason)
        {
            ActiveProfile = XrSmokeProfile.Disabled;
            LastStatus = "XR UNAVAILABLE: " + reason;
            Debug.LogError("[MC2 XR SMOKE] " + LastStatus + "; owner-data=none; simulation=none");
        }
    }
}
