using System;
using UnityEngine;
using UnityEngine.UI;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// The player-facing Classic HD presentation route.  It is a 2D indexed
    /// display fed by a reusable frame plane, not a managed 3D world.
    /// </summary>
    public sealed class ClassicHdPresentation : MonoBehaviour
    {
        private ClassicHdR8Surface surface;
        private byte[] frameBytes;
        private Canvas canvas;
        private Image letterbox;
        private RawImage display;
        private ClassicHdPresentationSettings settings;
        private ClassicHdNativeFixtureSource nativeFixture;
        private ClassicHdOwnerSessionSource ownerSession;
        private Rect lastViewport;
        private int lastScreenWidth;
        private int lastScreenHeight;
        private ulong tick;
        private ulong resetEpoch;
        private Vector2 simulatedFlightInput;

        public bool IsConfigured { get; private set; }
        public string StatusLine { get; private set; }
        public int AllocationEpoch { get { return surface == null ? 0 : surface.AllocationEpoch; } }
        public int UploadCount { get { return surface == null ? 0 : surface.UploadCount; } }
        public ulong ResetEpoch { get { return resetEpoch; } }
        public ClassicHdPresentationSettings Settings { get { return settings; } }
        public bool UsesNativeFixture { get { return nativeFixture != null && nativeFixture.IsConfigured; } }
        public bool UsesNativeOwnerSession { get { return ownerSession != null && ownerSession.IsConfigured; } }
        public ulong SourceGeneration
        {
            get { return ownerSession == null ? 0 : ownerSession.SourceGeneration; }
        }
        public ulong SourceTick
        {
            get { return ownerSession == null ? 0 : ownerSession.SourceTick; }
        }
        public ulong LastNativeIndexHash
        {
            get
            {
                return ownerSession != null && ownerSession.IsConfigured
                    ? ownerSession.LastIndexHash
                    : (nativeFixture == null ? 0 : nativeFixture.LastIndexHash);
            }
        }
        public ulong LastNativeProvenanceHash
        {
            get
            {
                return ownerSession != null && ownerSession.IsConfigured
                    ? ownerSession.LastProvenanceHash
                    : (nativeFixture == null ? 0 : nativeFixture.LastProvenanceHash);
            }
        }
        public ulong LastNativeReceiptHash
        {
            get
            {
                return ownerSession != null && ownerSession.IsConfigured
                    ? ownerSession.LastReceiptHash
                    : (nativeFixture == null ? 0 : nativeFixture.LastReceiptHash);
            }
        }
        public uint LastNativeFlags
        {
            get
            {
                return ownerSession != null && ownerSession.IsConfigured
                    ? ownerSession.LastFlags
                    : (nativeFixture == null ? 0 : nativeFixture.LastFlags);
            }
        }
        public int PlayerLife
        {
            get { return ownerSession == null ? 0 : ownerSession.PlayerLife; }
        }
        public int PlayerMaxLife
        {
            get { return ownerSession == null ? 0 : ownerSession.PlayerMaxLife; }
        }
        public bool PlayerDead
        {
            get { return ownerSession != null && ownerSession.PlayerDead; }
        }
        public int MissionProjectileCount
        {
            get { return ownerSession == null ? 0 : ownerSession.MissionProjectileCount; }
        }
        public int MissionImpactCount
        {
            get { return ownerSession == null ? 0 : ownerSession.MissionImpactCount; }
        }
        public short ActualSpeed
        {
            get { return ownerSession == null ? (short)0 : ownerSession.ActualSpeed; }
        }
        public ushort CameraX
        {
            get { return ownerSession == null ? (ushort)0 : ownerSession.CameraX; }
        }
        public ushort CameraY
        {
            get { return ownerSession == null ? (ushort)0 : ownerSession.CameraY; }
        }

        public bool Configure(ClassicHdPresentationSettings requestedSettings, out string reason)
        {
            return ConfigureInternal(
                requestedSettings,
                false,
                false,
                string.Empty,
                string.Empty,
                string.Empty,
                out reason);
        }

        public bool ConfigureNativeFixture(ClassicHdPresentationSettings requestedSettings, out string reason)
        {
            return ConfigureInternal(requestedSettings, true, false, string.Empty, string.Empty, string.Empty, out reason);
        }

        public bool ConfigureOwnerSession(
            ClassicHdPresentationSettings requestedSettings,
            string packagePath,
            string closurePath,
            string assetCachePath,
            out string reason)
        {
            return ConfigureInternal(
                requestedSettings,
                false,
                true,
                packagePath,
                closurePath,
                assetCachePath,
                out reason);
        }

        private bool ConfigureInternal(
            ClassicHdPresentationSettings requestedSettings,
            bool requireNativeFixture,
            bool requireOwnerSession,
            string packagePath,
            string closurePath,
            string assetCachePath,
            out string reason)
        {
            reason = string.Empty;
            if (requestedSettings == null)
            {
                reason = "Classic HD presentation settings are null";
                return false;
            }

            string validationError;
            if (!requestedSettings.TryValidate(out validationError))
            {
                reason = validationError;
                return false;
            }

            ClassicHdProfileInfo profileInfo;
            ClassicHdPresentationSettings.TryGetProfile(requestedSettings.profile, out profileInfo);
            if (surface == null)
            {
                surface = new ClassicHdR8Surface();
            }

            if (!surface.Configure(requestedSettings, out reason))
            {
                return false;
            }

            settings = requestedSettings.Clone();
            if (frameBytes == null || frameBytes.Length != profileInfo.Stride * profileInfo.Height)
            {
                frameBytes = new byte[profileInfo.Stride * profileInfo.Height];
            }

            if (nativeFixture != null)
            {
                nativeFixture.Dispose();
                nativeFixture = null;
            }

            if (ownerSession != null)
            {
                ownerSession.Dispose();
                ownerSession = null;
            }

            if (requireNativeFixture)
            {
                nativeFixture = new ClassicHdNativeFixtureSource();
                if (!nativeFixture.Configure(profileInfo, out reason))
                {
                    nativeFixture.Dispose();
                    nativeFixture = null;
                    return false;
                }
            }

            if (requireOwnerSession)
            {
                ownerSession = new ClassicHdOwnerSessionSource();
                if (!ownerSession.Configure(
                        profileInfo,
                        requestedSettings,
                        packagePath,
                        closurePath,
                        assetCachePath,
                        out reason))
                {
                    ownerSession.Dispose();
                    ownerSession = null;
                    return false;
                }
            }

            CreateDisplayObjects();
            IsConfigured = true;
            StatusLine = "Classic HD " + settings.profile + " / indexed R8 / " +
                        (UsesNativeOwnerSession
                             ? "owner-bound Rust profile-sized Classic HD rerender"
                             : (UsesNativeFixture ? "native additive ABI fixture" : "synthetic lab source"));
            UpdateDisplayViewport(true);
            ResetPresentationHistory();
            return true;
        }

        public void ResetPresentationHistory()
        {
            tick = 0;
            resetEpoch++;
            simulatedFlightInput = Vector2.zero;
            if (surface != null)
            {
                surface.Clear();
            }
        }

        public bool ResetOwnerSession(out string reason)
        {
            reason = string.Empty;
            if (!UsesNativeOwnerSession)
            {
                reason = "Classic HD owner session is not configured";
                return false;
            }

            if (!ownerSession.Reset(out reason))
            {
                return false;
            }

            ResetPresentationHistory();
            return true;
        }

        public bool InjectPlayerDamageForQualification(
            int damage,
            ushort attackerEntityIndex,
            out string reason)
        {
            reason = string.Empty;
            if (ownerSession == null || !ownerSession.IsConfigured)
            {
                reason = "Classic HD owner session is not configured";
                return false;
            }

            return ownerSession.InjectPlayerDamageForQualification(
                damage,
                attackerEntityIndex,
                out reason);
        }

        public bool CaptureFrame(float horizontalInput, float verticalInput, out string reason)
        {
            return CaptureFrame(horizontalInput, verticalInput, false, -1, out reason);
        }

        public bool CaptureFrame(
            float horizontalInput,
            float verticalInput,
            bool cast,
            int spellSelection,
            out string reason)
        {
            reason = string.Empty;
            if (!IsConfigured || surface == null || frameBytes == null)
            {
                reason = "Classic HD presentation is not configured";
                return false;
            }

            tick++;
            if (UsesNativeOwnerSession)
            {
                if (!ownerSession.Fill(
                        frameBytes,
                        horizontalInput,
                        verticalInput,
                        cast,
                        spellSelection,
                        out reason))
                {
                    return false;
                }
            }
            else if (UsesNativeFixture)
            {
                if (!nativeFixture.Fill(
                        frameBytes,
                        1,
                        tick,
                        resetEpoch,
                        InputHash(horizontalInput, verticalInput),
                        out reason))
                {
                    return false;
                }
            }
            else
            {
                SyntheticClassicHdFrameSource.Fill(
                    frameBytes,
                    surface.Width,
                    surface.Height,
                    tick,
                    horizontalInput,
                    verticalInput);
            }

            string uploadError;
            if (!surface.UploadTopRowFirst(frameBytes, surface.Stride, out uploadError))
            {
                reason = uploadError;
                return false;
            }

            UpdateDisplayViewport(false);
            return true;
        }

        private void Update()
        {
            if (!IsConfigured || surface == null || frameBytes == null)
            {
                return;
            }

            // This input is presentation-only in the lab; a production player
            // supplies the same sampled input to Rust and this renderer.
            simulatedFlightInput.x = Input.GetAxisRaw("Horizontal");
            simulatedFlightInput.y = Input.GetAxisRaw("Vertical");
            string captureError;
            if (!CaptureFrame(simulatedFlightInput.x, simulatedFlightInput.y, out captureError))
            {
                StatusLine = "Classic HD capture stopped: " + captureError;
                return;
            }
        }

        private void OnDestroy()
        {
            if (surface != null)
            {
                surface.Dispose();
                surface = null;
            }

            if (nativeFixture != null)
            {
                nativeFixture.Dispose();
                nativeFixture = null;
            }

            if (ownerSession != null)
            {
                ownerSession.Dispose();
                ownerSession = null;
            }
        }

        private static ulong InputHash(float horizontal, float vertical)
        {
            unchecked
            {
                int x = Mathf.RoundToInt(horizontal * 1000f);
                int y = Mathf.RoundToInt(vertical * 1000f);
                ulong hash = 0xcbf29ce484222325UL;
                hash ^= (uint)x;
                hash *= 0x100000001b3UL;
                hash ^= (uint)y;
                hash *= 0x100000001b3UL;
                return hash;
            }
        }

        private void CreateDisplayObjects()
        {
            if (canvas == null)
            {
                GameObject canvasObject = new GameObject("Classic HD Display Canvas");
                canvasObject.transform.SetParent(transform, false);
                canvas = canvasObject.AddComponent<Canvas>();
                canvas.renderMode = RenderMode.ScreenSpaceOverlay;
                canvas.sortingOrder = -100;
                CanvasScaler scaler = canvasObject.AddComponent<CanvasScaler>();
                scaler.uiScaleMode = CanvasScaler.ScaleMode.ConstantPixelSize;
                scaler.scaleFactor = 1f;

                GameObject letterboxObject = new GameObject("Classic HD Letterbox");
                letterboxObject.transform.SetParent(canvasObject.transform, false);
                letterbox = letterboxObject.AddComponent<Image>();
                letterbox.color = new Color32(2, 4, 10, 255);
                letterbox.raycastTarget = false;
                RectTransform letterboxRect = letterbox.rectTransform;
                letterboxRect.anchorMin = Vector2.zero;
                letterboxRect.anchorMax = Vector2.one;
                letterboxRect.offsetMin = Vector2.zero;
                letterboxRect.offsetMax = Vector2.zero;

                GameObject displayObject = new GameObject("Classic HD Indexed Display");
                displayObject.transform.SetParent(canvasObject.transform, false);
                display = displayObject.AddComponent<RawImage>();
                display.raycastTarget = false;
            }

            display.texture = surface.IndexTexture;
            display.material = surface.Material;
            display.color = Color.white;
        }

        private void UpdateDisplayViewport(bool force)
        {
            if (display == null || surface == null)
            {
                return;
            }

            if (!force && lastScreenWidth == Screen.width && lastScreenHeight == Screen.height)
            {
                return;
            }

            lastScreenWidth = Screen.width;
            lastScreenHeight = Screen.height;
            lastViewport = ClassicHdDisplayComposition.CalculateViewport(
                Screen.width, Screen.height, surface.Width, surface.Height);
            ClassicHdDisplayComposition.ApplyTo(display.rectTransform, lastViewport);
        }
    }

    internal static class SyntheticClassicHdFrameSource
    {
        public static void Fill(
            byte[] destination,
            int width,
            int height,
            ulong tick,
            float horizontalInput,
            float verticalInput)
        {
            if (destination == null || width <= 0 || height <= 0 || destination.Length < width * height)
            {
                return;
            }

            int horizon = height * 3 / 5;
            int drift = ((int)(tick / 3) + Mathf.RoundToInt(horizontalInput * 9f)) % width;
            if (drift < 0)
            {
                drift += width;
            }

            for (int y = 0; y < height; y++)
            {
                int row = y * width;
                if (y < horizon)
                {
                    int sky = 14 + (horizon - y) * 22 / Mathf.Max(1, horizon);
                    for (int x = 0; x < width; x++)
                    {
                        destination[row + x] = (byte)Mathf.Clamp(sky + ((x + drift) % 19) / 7, 8, 55);
                    }
                }
                else
                {
                    int ground = 44 + (y - horizon) * 80 / Mathf.Max(1, height - horizon);
                    for (int x = 0; x < width; x++)
                    {
                        int contour = ((x + drift) / 23 + y / 19) % 5;
                        destination[row + x] = (byte)Mathf.Clamp(ground + contour * 4, 42, 150);
                    }
                }
            }

            DrawCircle(destination, width, height, width * 7 / 8, height / 8, Mathf.Max(5, width / 44), 232);
            DrawStars(destination, width, height, tick);
            DrawRiver(destination, width, height, drift);
            DrawTown(destination, width, height, width / 2, horizon + height / 12);
            DrawFlightMarker(destination, width, height, horizontalInput, verticalInput);
        }

        private static void DrawStars(byte[] destination, int width, int height, ulong tick)
        {
            int count = Mathf.Max(20, width / 8);
            for (int star = 0; star < count; star++)
            {
                int x = (star * 97 + 31) % width;
                int y = (star * 47 + 19) % Mathf.Max(1, height / 2);
                if (((star + (int)(tick / 12)) & 3) == 0)
                {
                    destination[y * width + x] = 220;
                }
            }
        }

        private static void DrawRiver(byte[] destination, int width, int height, int drift)
        {
            int start = height * 3 / 5;
            for (int y = start; y < height; y++)
            {
                int centre = width / 2 + (y - start) * (drift % 17 - 8) / Mathf.Max(1, height - start);
                int halfWidth = Mathf.Max(3, width / 38 + (y - start) / 18);
                int row = y * width;
                for (int x = Mathf.Max(0, centre - halfWidth); x < Mathf.Min(width, centre + halfWidth); x++)
                {
                    destination[row + x] = 118;
                }
            }
        }

        private static void DrawTown(byte[] destination, int width, int height, int centreX, int baseY)
        {
            int buildingWidth = Mathf.Max(4, width / 30);
            int buildingHeight = Mathf.Max(6, height / 16);
            for (int building = -4; building <= 4; building++)
            {
                int x = centreX + building * buildingWidth * 2;
                for (int y = Mathf.Max(0, baseY - buildingHeight); y < Mathf.Min(height, baseY); y++)
                {
                    int row = y * width;
                    for (int column = 0; column < buildingWidth; column++)
                    {
                        int pixelX = x + column;
                        if (pixelX >= 0 && pixelX < width)
                        {
                            destination[row + pixelX] = (byte)(building % 2 == 0 ? 173 : 160);
                        }
                    }
                }
            }
        }

        private static void DrawFlightMarker(byte[] destination, int width, int height, float horizontal, float vertical)
        {
            int centreX = width / 2 + Mathf.RoundToInt(horizontal * width / 7f);
            int centreY = height / 2 + Mathf.RoundToInt(vertical * height / 10f);
            for (int offset = -2; offset <= 2; offset++)
            {
                int x = centreX + offset;
                int y = centreY + offset;
                if (x >= 0 && x < width && centreY >= 0 && centreY < height)
                {
                    destination[centreY * width + x] = 245;
                }

                if (y >= 0 && y < height && centreX >= 0 && centreX < width)
                {
                    destination[y * width + centreX] = 245;
                }
            }
        }

        private static void DrawCircle(byte[] destination, int width, int height, int centreX, int centreY, int radius, byte value)
        {
            int radiusSquared = radius * radius;
            for (int y = Mathf.Max(0, centreY - radius); y <= Mathf.Min(height - 1, centreY + radius); y++)
            {
                for (int x = Mathf.Max(0, centreX - radius); x <= Mathf.Min(width - 1, centreX + radius); x++)
                {
                    int dx = x - centreX;
                    int dy = y - centreY;
                    if (dx * dx + dy * dy <= radiusSquared)
                    {
                        destination[y * width + x] = value;
                    }
                }
            }
        }
    }
}
