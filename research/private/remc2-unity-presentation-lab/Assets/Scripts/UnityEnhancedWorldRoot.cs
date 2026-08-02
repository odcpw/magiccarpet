using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedLifecycleState
    {
        Closed = 0,
        Open = 1,
        Faulted = 2
    }

    [Serializable]
    public struct UnityEnhancedCameraPose
    {
        public Vector3 Position;
        public Quaternion Rotation;
        public float FieldOfView;

        public UnityEnhancedCameraPose(Vector3 position, Quaternion rotation, float fieldOfView)
        {
            Position = position;
            Rotation = rotation;
            FieldOfView = fieldOfView;
        }
    }

    [Serializable]
    public sealed class UnityEnhancedEntitySnapshot
    {
        public ulong StableIdentity;
        public ulong Generation;
        public Vector3 Position;
        public Quaternion Rotation = Quaternion.identity;
        public bool Visible = true;
        public int PresentationFamily;
        public float FacingDegrees;
        public float AnimationPhase;
        public float Scale = 1f;
        public int RenderLayer;
    }

    [Serializable]
    public sealed class UnityEnhancedOwnerHudFrame
    {
        public ulong Generation;
        public ulong Tick;
        public int Width;
        public int Height;
        public int Stride;
        public byte[] IndexedPixels;
        public byte[] Provenance;
        public byte[] PaletteRgb8;

        public bool TryValidate(ulong expectedGeneration, ulong expectedTick, out string reason)
        {
            reason = string.Empty;
            long pixelCount = (long)Stride * Height;
            if (Generation != expectedGeneration || Tick != expectedTick ||
                Width <= 0 || Height <= 0 || Stride != Width ||
                pixelCount <= 0 || pixelCount > int.MaxValue ||
                IndexedPixels == null || IndexedPixels.Length != pixelCount ||
                Provenance == null || Provenance.Length != pixelCount ||
                PaletteRgb8 == null || PaletteRgb8.Length != 256 * 3)
            {
                reason = "owner HUD frame identity, extent, provenance, or palette contract failed";
                return false;
            }

            return true;
        }
    }

    [Serializable]
    public sealed class UnityEnhancedHudSnapshot
    {
        public ulong Generation;
        public ulong Tick;
        public UnityEnhancedOwnerHudFrame OwnerFrame;
        public int PlayerLife;
        public int PlayerMaxLife;
        public int PlayerMana;
        public int PlayerMaxMana;
        public int CastleMana;
        public int SecuredManaPercent;
        public int TargetSpeed;
        public int ActualSpeed;
        public byte CurrentStage;
        public byte TasksCompleted;
        public byte ObjectiveSelector;
        public byte ObjectiveTextboxTicks;
        public byte ObjectiveCount;
        public byte LeftSpellSlot;
        public byte RightSpellSlot;
        public byte SelectedSpellUiIndex;
        public byte SpeedTrailCount;
        public bool HasFireball;
        public bool HasPossession;
        public bool HasCastleSpell;
        public bool HasSpeedSpell;
        public bool PlayerDead;
        public bool ExitSequenceActive;
        public bool ExitSequenceComplete;
        public byte ExitResultCode;
    }

    [Serializable]
    public sealed class UnityEnhancedWorldSnapshot
    {
        public ulong Generation;
        public ulong Tick;
        public bool Reset;
        public UnityEnhancedCameraPose CameraPose;
        public UnityEnhancedEntitySnapshot[] Entities = Array.Empty<UnityEnhancedEntitySnapshot>();
        public UnityEnhancedHudSnapshot Hud;
        public UnityEnhancedMinimapSnapshot Minimap;
        public UnityEnhancedTerrainSnapshot Terrain;
        public UnityEnhancedWaterSnapshot Water;
        public UnityEnhancedNightLightingDescriptor? NightLighting;
        public UnityEnhancedLocalLightRequest[] LocalLights = Array.Empty<UnityEnhancedLocalLightRequest>();
        public UnityEnhancedVfxEvent[] VfxEvents = Array.Empty<UnityEnhancedVfxEvent>();
        public UnityEnhancedQualitySettings Quality;
    }

    internal struct UnityEnhancedEntityPose
    {
        public Vector3 Position;
        public Quaternion Rotation;

        public UnityEnhancedEntityPose(Vector3 position, Quaternion rotation)
        {
            Position = position;
            Rotation = rotation;
        }
    }

    /// <summary>
    /// Owns only Unity Enhanced presentation objects. Gameplay state remains in
    /// the Rust session and arrives as immutable generation-bound snapshots.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedWorldRoot : MonoBehaviour
    {
        private readonly Dictionary<ulong, GameObject> activeEntities = new Dictionary<ulong, GameObject>();
        private readonly Stack<GameObject> pooledEntities = new Stack<GameObject>();
        private readonly HashSet<ulong> seenIdentities = new HashSet<ulong>();
        private readonly Dictionary<ulong, UnityEnhancedEntityPose> entityStartPoses = new Dictionary<ulong, UnityEnhancedEntityPose>();
        private readonly Dictionary<ulong, UnityEnhancedEntityPose> entityTargetPoses = new Dictionary<ulong, UnityEnhancedEntityPose>();

        private Transform worldObjects;
        private UnityEnhancedCameraRig cameraRig;
        private UnityEnhancedPresentationAdapters presentationAdapters;
        private UnityEnhancedTerrainPresentation terrainPresentation;
        private UnityEnhancedActorPresentation actorPresentation;
        private UnityEnhancedNightLighting nightLighting;
        private UnityEnhancedWaterPresentation waterPresentation;
        private UnityEnhancedVfxPresentation vfxPresentation;
        private UnityEnhancedQualityAndPost qualityAndPost;
        private UnityEnhancedHudPresentation hudPresentation;
        private UnityEnhancedDiagnostics diagnostics;
        private ulong generation;
        private ulong lastTick;
        private bool hasSnapshot;

        public UnityEnhancedLifecycleState State { get; private set; }
        public string LastRejectReason { get; private set; }
        public ulong Generation { get { return generation; } }
        public ulong LastTick { get { return lastTick; } }
        public int ActiveEntityCount { get { return activeEntities.Count; } }
        public int PooledEntityCount { get { return pooledEntities.Count; } }
        public UnityEnhancedCameraRig CameraRig { get { return cameraRig; } }
        public UnityEnhancedInputAdapter InputAdapter { get; private set; }
        public UnityEnhancedPresentationAdapters PresentationAdapters { get { return presentationAdapters; } }
        public UnityEnhancedTerrainPresentation TerrainPresentation { get { return terrainPresentation; } }
        public UnityEnhancedActorPresentation ActorPresentation { get { return actorPresentation; } }
        public UnityEnhancedNightLighting NightLighting { get { return nightLighting; } }
        public UnityEnhancedWaterPresentation WaterPresentation { get { return waterPresentation; } }
        public UnityEnhancedVfxPresentation VfxPresentation { get { return vfxPresentation; } }
        public UnityEnhancedQualityAndPost QualityAndPost { get { return qualityAndPost; } }
        public UnityEnhancedHudPresentation HudPresentation { get { return hudPresentation; } }
        public UnityEnhancedDiagnostics Diagnostics { get { return diagnostics; } }

        private void Awake()
        {
            State = UnityEnhancedLifecycleState.Closed;
        }

        private void OnDestroy()
        {
            Close();
        }

        private void LateUpdate()
        {
            if (State == UnityEnhancedLifecycleState.Open && hasSnapshot)
            {
                if (diagnostics != null) diagnostics.BeginFrame();
                InterpolatePresentation(Mathf.Clamp01(Time.unscaledDeltaTime * 30f));
                if (vfxPresentation != null) vfxPresentation.Advance(lastTick);
                if (diagnostics != null)
                {
                    diagnostics.RecordEffects(vfxPresentation, nightLighting);
                    diagnostics.EndFrame();
                }
            }
        }

        /// <summary>
        /// Opens the shell after the launcher has selected Enhanced. This
        /// overload is intentionally independent of owner data so synthetic
        /// diagnostics can exercise the presentation lifecycle.
        /// </summary>
        public void OpenDiagnostic(PresentationModeId selectedMode)
        {
            OpenInternal(selectedMode);
        }

        /// <summary>
        /// Opens the shell for a validated presentation session. The lease is
        /// checked here, before any Enhanced hierarchy is assembled.
        /// </summary>
        public void Open(PresentationModeId selectedMode, PresentationSessionLease sessionLease)
        {
            if (sessionLease == null || !sessionLease.IsOpen || sessionLease.Mode != selectedMode)
            {
                throw new InvalidOperationException("Unity Enhanced requires an open matching presentation session");
            }

            OpenInternal(selectedMode);
        }

        public void Close()
        {
            if (State == UnityEnhancedLifecycleState.Closed)
            {
                return;
            }

            ClearActiveEntities();
            DestroyPooledEntities();
            if (InputAdapter != null)
            {
                InputAdapter.ResetForSession();
            }

            if (presentationAdapters != null)
            {
                presentationAdapters.Reset(0);
            }

            if (terrainPresentation != null) terrainPresentation.ResetPresentation(0);
            if (waterPresentation != null) waterPresentation.ResetPresentation(0);
            if (nightLighting != null) nightLighting.ResetForSession();
            if (vfxPresentation != null) vfxPresentation.ResetForGeneration(0);
            if (actorPresentation != null) actorPresentation.ResetForSession();
            if (qualityAndPost != null) qualityAndPost.ResetForSession();
            if (hudPresentation != null) hudPresentation.ResetForSession();
            if (diagnostics != null) diagnostics.ResetForSession();

            hasSnapshot = false;
            generation = 0;
            lastTick = 0;
            LastRejectReason = string.Empty;
            State = UnityEnhancedLifecycleState.Closed;
        }

        /// <summary>
        /// Applies a complete immutable presentation snapshot. Invalid or stale
        /// input is rejected before mutating the current world.
        /// </summary>
        public bool ApplySnapshot(UnityEnhancedWorldSnapshot snapshot)
        {
            LastRejectReason = string.Empty;
            if (State != UnityEnhancedLifecycleState.Open || snapshot == null)
            {
                LastRejectReason = snapshot == null
                    ? "snapshot was null"
                    : "Enhanced root was not open";
                return false;
            }

            if (!ValidateSnapshot(snapshot))
            {
                LastRejectReason = "snapshot identity or generation validation failed";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }

            if (hasSnapshot && snapshot.Generation == generation && snapshot.Tick < lastTick)
            {
                LastRejectReason = "snapshot tick regressed within its generation";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                return false;
            }

            if (!hasSnapshot || snapshot.Reset || snapshot.Generation != generation)
            {
                ResetPresentation(snapshot.Generation);
            }

            if (snapshot.Terrain != null && !terrainPresentation.ApplySnapshot(snapshot.Terrain))
            {
                LastRejectReason = "terrain presentation rejected snapshot";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }

            if (snapshot.Water != null && !waterPresentation.ApplySnapshot(snapshot.Water))
            {
                LastRejectReason = "water presentation rejected snapshot";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }

            if (snapshot.NightLighting.HasValue)
            {
                nightLighting.ApplyDescriptor(
                    snapshot.Generation,
                    snapshot.NightLighting.Value,
                    snapshot.LocalLights,
                    snapshot.Reset,
                    cameraRig.PresentationCamera);
            }

            float sceneExposure = snapshot.NightLighting.HasValue
                ? snapshot.NightLighting.Value.Exposure
                : 0f;
            if (snapshot.Quality != null && !qualityAndPost.Apply(
                    snapshot.Quality,
                    cameraRig.PresentationCamera,
                    sceneExposure))
            {
                LastRejectReason = "quality/post presentation rejected snapshot";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }
            if (snapshot.Quality != null)
            {
                vfxPresentation.SetReducedMotion(snapshot.Quality.ReducedMotion);
                vfxPresentation.SetCameraComfort(snapshot.Quality.CameraShake);
                diagnostics.SetAccessibility(snapshot.Quality.ReducedMotion, false);
            }

            if (snapshot.Hud != null && !hudPresentation.ApplySnapshot(
                    snapshot.Hud,
                    cameraRig.PresentationCamera))
            {
                LastRejectReason = "HUD presentation rejected snapshot";
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }
            if (snapshot.Hud == null)
            {
                hudPresentation.HideForSnapshot();
            }

            seenIdentities.Clear();
            for (int index = 0; index < snapshot.Entities.Length; index++)
            {
                UnityEnhancedEntitySnapshot entity = snapshot.Entities[index];
                seenIdentities.Add(entity.StableIdentity);

                GameObject view;
                if (!entity.Visible)
                {
                    if (activeEntities.TryGetValue(entity.StableIdentity, out view))
                    {
                        activeEntities.Remove(entity.StableIdentity);
                        entityStartPoses.Remove(entity.StableIdentity);
                        entityTargetPoses.Remove(entity.StableIdentity);
                        Recycle(view);
                    }

                    continue;
                }

                bool isNewEntity = false;
                if (!activeEntities.TryGetValue(entity.StableIdentity, out view))
                {
                    view = Acquire(entity.StableIdentity);
                    activeEntities.Add(entity.StableIdentity, view);
                    isNewEntity = true;
                }

                UnityEnhancedEntityPose targetPose = new UnityEnhancedEntityPose(entity.Position, entity.Rotation);
                UnityEnhancedEntityPose startPose;
                if (isNewEntity)
                {
                    startPose = targetPose;
                    view.transform.SetPositionAndRotation(entity.Position, entity.Rotation);
                }
                else
                {
                    startPose = new UnityEnhancedEntityPose(view.transform.position, view.transform.rotation);
                }

                entityStartPoses[entity.StableIdentity] = startPose;
                entityTargetPoses[entity.StableIdentity] = targetPose;
                actorPresentation.Configure(view, entity);
                view.name = "EnhancedEntity_" + entity.StableIdentity.ToString("x16");
                view.SetActive(true);
            }

            List<ulong> staleIdentities = new List<ulong>();
            foreach (KeyValuePair<ulong, GameObject> pair in activeEntities)
            {
                if (!seenIdentities.Contains(pair.Key))
                {
                    staleIdentities.Add(pair.Key);
                }
            }

            for (int index = 0; index < staleIdentities.Count; index++)
            {
                ulong identity = staleIdentities[index];
                GameObject stale = activeEntities[identity];
                activeEntities.Remove(identity);
                entityStartPoses.Remove(identity);
                entityTargetPoses.Remove(identity);
                Recycle(stale);
            }

            cameraRig.SetTargetPose(snapshot.CameraPose);
            presentationAdapters.ApplyTick(snapshot.Generation, snapshot.Tick);
            for (int index = 0; index < snapshot.VfxEvents.Length; index++)
            {
                vfxPresentation.ApplyEvent(snapshot.VfxEvents[index]);
            }
            vfxPresentation.Advance(snapshot.Tick);
            generation = snapshot.Generation;
            lastTick = snapshot.Tick;
            hasSnapshot = true;
            if (diagnostics != null) diagnostics.RecordSnapshot(snapshot, this);
            return true;
        }

        /// <summary>
        /// Advances only Unity presentation transforms toward the latest
        /// snapshot. It is intentionally separate from snapshot acceptance so
        /// render cadence cannot change Rust tick or command order.
        /// </summary>
        public void InterpolatePresentation(float alpha)
        {
            if (State != UnityEnhancedLifecycleState.Open)
            {
                return;
            }

            alpha = Mathf.Clamp01(alpha);
            foreach (KeyValuePair<ulong, GameObject> pair in activeEntities)
            {
                UnityEnhancedEntityPose startPose;
                UnityEnhancedEntityPose targetPose;
                if (!entityStartPoses.TryGetValue(pair.Key, out startPose) ||
                    !entityTargetPoses.TryGetValue(pair.Key, out targetPose))
                {
                    continue;
                }

                pair.Value.transform.SetPositionAndRotation(
                    Vector3.Lerp(startPose.Position, targetPose.Position, alpha),
                    Quaternion.Slerp(startPose.Rotation, targetPose.Rotation, alpha));
            }

            cameraRig.InterpolateToTarget(alpha);
        }

        public void ResetPresentation(ulong nextGeneration)
        {
            ClearActiveEntities();
            generation = nextGeneration;
            lastTick = 0;
            hasSnapshot = false;
            LastRejectReason = string.Empty;
            State = UnityEnhancedLifecycleState.Open;
            presentationAdapters.Reset(nextGeneration);
            terrainPresentation.ResetPresentation(nextGeneration);
            waterPresentation.ResetPresentation(nextGeneration);
            nightLighting.ResetForGeneration(nextGeneration);
            vfxPresentation.ResetForGeneration(nextGeneration);
            actorPresentation.ResetForSession();
            hudPresentation.ResetForGeneration(nextGeneration);
        }

        /// <summary>
        /// Captures the current Enhanced camera for qualification tooling.
        /// This renders the already-applied presentation snapshot only; it
        /// never advances or mutates the authoritative owner session.
        /// </summary>
        public bool CapturePresentationFramePng(
            int width,
            int height,
            out byte[] pngBytes,
            out ulong resolvedHash,
            out string reason)
        {
            pngBytes = null;
            resolvedHash = 0;
            reason = string.Empty;
            if (State != UnityEnhancedLifecycleState.Open || cameraRig == null ||
                cameraRig.PresentationCamera == null || !hasSnapshot)
            {
                reason = "Unity Enhanced camera is not ready for resolved capture";
                return false;
            }

            if (width <= 0 || height <= 0 || width > 4096 || height > 4096)
            {
                reason = "Unity Enhanced capture dimensions are outside the bounded range";
                return false;
            }

            Camera camera = cameraRig.PresentationCamera;
            RenderTexture target = new RenderTexture(
                width,
                height,
                24,
                RenderTextureFormat.ARGB32,
                // Enhanced resolves through the display-space color path.
                // Pair the sRGB target with a linear readback texture so the
                // PNG records those resolved pixels without a second gamma
                // decode in Gamma-color-space qualification projects.
                RenderTextureReadWrite.sRGB);
            target.name = "UnityEnhanced_QualificationResolvedCapture";
            Texture2D readback = null;
            RenderTexture previousActive = RenderTexture.active;
            RenderTexture previousTarget = camera.targetTexture;
            try
            {
                target.Create();
                // Editor/batch qualification does not get a LateUpdate pass
                // between ApplySnapshot and capture. Advance only the
                // presentation interpolation so the camera actually renders
                // the latest accepted target pose.
                InterpolatePresentation(1f);
                camera.targetTexture = target;
                camera.Render();
                if (hudPresentation == null ||
                    !hudPresentation.CompositeOwnerHudTo(target, out reason))
                {
                    return false;
                }

                RenderTexture.active = target;
                readback = new Texture2D(width, height, TextureFormat.RGBA32, false, true);
                readback.name = "UnityEnhanced_QualificationResolvedReadback";
                readback.ReadPixels(new Rect(0, 0, width, height), 0, 0, false);
                readback.Apply(false, false);
                pngBytes = readback.EncodeToPNG();
                if (pngBytes == null || pngBytes.Length == 0)
                {
                    reason = "resolved Unity Enhanced capture produced an empty PNG";
                    pngBytes = null;
                    return false;
                }

                resolvedHash = HashBytes(pngBytes);
                return true;
            }
            catch (Exception exception)
            {
                reason = "resolved Unity Enhanced capture failed: " + exception.Message;
                pngBytes = null;
                resolvedHash = 0;
                return false;
            }
            finally
            {
                camera.targetTexture = previousTarget;
                RenderTexture.active = previousActive;
                if (readback != null)
                {
                    UnityEngine.Object.DestroyImmediate(readback);
                }

                target.Release();
                UnityEngine.Object.DestroyImmediate(target);
            }
        }

        private void OpenInternal(PresentationModeId selectedMode)
        {
            if (selectedMode != PresentationModeId.UnityEnhanced)
            {
                throw new InvalidOperationException("Unity Enhanced cannot open for a non-Enhanced mode");
            }

            bool openingNewSession = State != UnityEnhancedLifecycleState.Open;
            EnsureHierarchy();
            if (openingNewSession)
            {
                InputAdapter.ResetForSession();
                presentationAdapters.Reset(0);
                diagnostics.ResetForSession();
                qualityAndPost.Apply(UnityEnhancedQualitySettings.ForTier(UnityEnhancedQualityTier.WholeHog), cameraRig.PresentationCamera);
            }
            State = UnityEnhancedLifecycleState.Open;
        }

        private void EnsureHierarchy()
        {
            if (worldObjects == null)
            {
                Transform existing = transform.Find("EnhancedWorldObjects");
                if (existing != null)
                {
                    worldObjects = existing;
                }
                else
                {
                    GameObject world = new GameObject("EnhancedWorldObjects");
                    world.transform.SetParent(transform, false);
                    worldObjects = world.transform;
                }
            }

            if (cameraRig == null)
            {
                cameraRig = GetComponentInChildren<UnityEnhancedCameraRig>();
                if (cameraRig == null)
                {
                    GameObject cameraObject = new GameObject("EnhancedCameraRig");
                    cameraObject.transform.SetParent(transform, false);
                    cameraRig = cameraObject.AddComponent<UnityEnhancedCameraRig>();
                }
            }
            cameraRig.EnsureCamera();

            if (InputAdapter == null)
            {
                InputAdapter = GetComponent<UnityEnhancedInputAdapter>();
                if (InputAdapter == null)
                {
                    InputAdapter = gameObject.AddComponent<UnityEnhancedInputAdapter>();
                }
            }

            if (presentationAdapters == null)
            {
                presentationAdapters = GetComponent<UnityEnhancedPresentationAdapters>();
                if (presentationAdapters == null)
                {
                    presentationAdapters = gameObject.AddComponent<UnityEnhancedPresentationAdapters>();
                }
            }

            if (terrainPresentation == null)
            {
                GameObject terrain = new GameObject("EnhancedTerrain");
                terrain.transform.SetParent(worldObjects, false);
                terrain.hideFlags = HideFlags.DontSave;
                terrainPresentation = terrain.AddComponent<UnityEnhancedTerrainPresentation>();
            }

            if (actorPresentation == null)
            {
                actorPresentation = GetComponent<UnityEnhancedActorPresentation>();
                if (actorPresentation == null) actorPresentation = gameObject.AddComponent<UnityEnhancedActorPresentation>();
            }

            if (nightLighting == null)
            {
                nightLighting = GetComponent<UnityEnhancedNightLighting>();
                if (nightLighting == null) nightLighting = gameObject.AddComponent<UnityEnhancedNightLighting>();
            }

            if (waterPresentation == null)
            {
                GameObject water = new GameObject("EnhancedWater");
                water.transform.SetParent(worldObjects, false);
                water.hideFlags = HideFlags.DontSave;
                waterPresentation = water.AddComponent<UnityEnhancedWaterPresentation>();
            }

            if (vfxPresentation == null)
            {
                vfxPresentation = GetComponent<UnityEnhancedVfxPresentation>();
                if (vfxPresentation == null) vfxPresentation = gameObject.AddComponent<UnityEnhancedVfxPresentation>();
            }

            if (hudPresentation == null)
            {
                GameObject hud = new GameObject("EnhancedHud");
                hud.transform.SetParent(transform, false);
                hud.hideFlags = HideFlags.DontSave;
                hudPresentation = hud.AddComponent<UnityEnhancedHudPresentation>();
            }

            if (qualityAndPost == null)
            {
                qualityAndPost = GetComponent<UnityEnhancedQualityAndPost>();
                if (qualityAndPost == null) qualityAndPost = gameObject.AddComponent<UnityEnhancedQualityAndPost>();
            }

            if (diagnostics == null)
            {
                diagnostics = GetComponent<UnityEnhancedDiagnostics>();
                if (diagnostics == null) diagnostics = gameObject.AddComponent<UnityEnhancedDiagnostics>();
            }
        }

        private bool ValidateSnapshot(UnityEnhancedWorldSnapshot snapshot)
        {
            if (snapshot.Generation == 0 ||
                (snapshot.Reset && snapshot.Tick != 0) ||
                !IsFinite(snapshot.CameraPose.Position) ||
                !IsFinite(snapshot.CameraPose.Rotation) ||
                !IsFinite(snapshot.CameraPose.FieldOfView) ||
                snapshot.CameraPose.FieldOfView <= 1f ||
                snapshot.CameraPose.FieldOfView >= 179f ||
                snapshot.Entities == null ||
                snapshot.LocalLights == null ||
                snapshot.VfxEvents == null)
            {
                return false;
            }

            seenIdentities.Clear();
            for (int index = 0; index < snapshot.Entities.Length; index++)
            {
                UnityEnhancedEntitySnapshot entity = snapshot.Entities[index];
                if (entity == null || entity.StableIdentity == 0 ||
                    entity.Generation != snapshot.Generation ||
                    !UnityEnhancedActorPresentation.HasCoverage(entity.PresentationFamily) ||
                    !IsFinite(entity.Position) || !IsFinite(entity.Rotation) ||
                    !IsFinite(entity.FacingDegrees) || !IsFinite(entity.AnimationPhase) ||
                    !IsFinite(entity.Scale) || entity.Scale <= 0f)
                {
                    return false;
                }

                if (!seenIdentities.Add(entity.StableIdentity))
                {
                    return false;
                }
            }

            if (snapshot.Hud != null &&
                (snapshot.Hud.Generation != snapshot.Generation ||
                 snapshot.Hud.Tick != snapshot.Tick ||
                 snapshot.Hud.OwnerFrame == null ||
                 !snapshot.Hud.OwnerFrame.TryValidate(
                     snapshot.Generation,
                     snapshot.Tick,
                     out string ownerHudFrameReason) ||
                 snapshot.Hud.PlayerLife < 0 ||
                 snapshot.Hud.PlayerMaxLife < 0 ||
                 snapshot.Hud.PlayerMana < 0 ||
                 snapshot.Hud.PlayerMaxMana < 0 ||
                 snapshot.Hud.SecuredManaPercent < 0 ||
                 snapshot.Hud.SecuredManaPercent > 100))
            {
                return false;
            }

            if (snapshot.Minimap != null)
            {
                string minimapReason;
                if (!snapshot.Minimap.TryValidate(snapshot.Generation, snapshot.Tick, out minimapReason))
                {
                    return false;
                }
            }

            if (snapshot.Terrain != null)
            {
                UnityEnhancedTerrainSnapshot terrain = snapshot.Terrain;
                long terrainCellCount = (long)terrain.Width * terrain.Height;
                if (terrain.Generation != snapshot.Generation ||
                    terrain.Tick != snapshot.Tick ||
                    terrain.Reset != snapshot.Reset ||
                    terrain.Width < 2 || terrain.Height < 2 ||
                    terrainCellCount > int.MaxValue ||
                    !IsFinite(terrain.Origin) ||
                    !IsFinite(terrain.CellSize) || terrain.CellSize <= 0f ||
                    terrain.Heights == null || terrain.TerrainTypes == null ||
                    terrain.Shade == null || terrain.Angles == null ||
                    terrain.DirtyRegions == null ||
                    terrain.Heights.Length != terrainCellCount ||
                    terrain.TerrainTypes.Length != terrainCellCount ||
                    terrain.Shade.Length != terrainCellCount ||
                    terrain.Angles.Length != terrainCellCount ||
                    (terrain.WrapMode != UnityEnhancedTerrainWrapMode.Clamp &&
                     terrain.WrapMode != UnityEnhancedTerrainWrapMode.Repeat))
                {
                    return false;
                }

                for (int index = 0; index < terrain.DirtyRegions.Length; index++)
                {
                    if (!IsValidDirtyRegion(terrain.DirtyRegions[index], terrain.Width, terrain.Height))
                    {
                        return false;
                    }
                }
            }

            if (snapshot.Water != null)
            {
                UnityEnhancedWaterSnapshot water = snapshot.Water;
                long shorelineCellCount = (long)water.ShorelineWidth * water.ShorelineHeight;
                if (water.Generation != snapshot.Generation ||
                    water.Tick != snapshot.Tick ||
                    water.Reset != snapshot.Reset ||
                    shorelineCellCount > int.MaxValue ||
                    !IsFinite(water.Center) || !IsFinite(water.Level) ||
                    !IsFinite(water.Size.x) || !IsFinite(water.Size.y) ||
                    water.Size.x <= 0f || water.Size.y <= 0f ||
                    !IsFinite(water.ShorelineCellSize) || water.ShorelineCellSize <= 0f ||
                    water.ShorelineWidth < 0 || water.ShorelineHeight < 0 ||
                    water.ShorelineMask == null ||
                    water.ShorelineMask.Length != shorelineCellCount)
                {
                    return false;
                }
            }

            seenIdentities.Clear();
            for (int index = 0; index < snapshot.LocalLights.Length; index++)
            {
                UnityEnhancedLocalLightRequest light = snapshot.LocalLights[index];
                if (light.StableIdentity == 0 || !IsFinite(light.Position) ||
                    !IsFinite(light.Color.r) || !IsFinite(light.Color.g) ||
                    !IsFinite(light.Color.b) || !IsFinite(light.Color.a) ||
                    !IsFinite(light.Intensity) || !IsFinite(light.Range) ||
                    light.Intensity < 0f || light.Range <= 0f)
                {
                    return false;
                }

                if (!seenIdentities.Add(light.StableIdentity))
                {
                    return false;
                }
            }

            ulong previousEffectSequence = 0;
            for (int index = 0; index < snapshot.VfxEvents.Length; index++)
            {
                UnityEnhancedVfxEvent effect = snapshot.VfxEvents[index];
                if (effect.Sequence == 0 || effect.Sequence <= previousEffectSequence ||
                    effect.Generation != snapshot.Generation || effect.Tick > snapshot.Tick ||
                    effect.StableIdentity == 0 || !UnityEnhancedVfxPresentation.HasCoverage(effect.Kind) ||
                    !IsFinite(effect.Position) || !IsFinite(effect.Intensity) || effect.Intensity < 0f)
                {
                    return false;
                }

                previousEffectSequence = effect.Sequence;
            }

            if (snapshot.Quality != null)
            {
                string qualityReason;
                if (!snapshot.Quality.TryValidate(out qualityReason))
                {
                    return false;
                }
            }

            return true;
        }

        private static bool IsFinite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        private static ulong HashBytes(byte[] bytes)
        {
            unchecked
            {
                ulong hash = 0xcbf29ce484222325UL;
                if (bytes == null)
                {
                    return hash;
                }

                for (int index = 0; index < bytes.Length; index++)
                {
                    hash ^= bytes[index];
                    hash *= 0x100000001b3UL;
                }

                return hash;
            }
        }

        private static bool IsFinite(Vector3 value)
        {
            return IsFinite(value.x) && IsFinite(value.y) && IsFinite(value.z);
        }

        private static bool IsFinite(Quaternion value)
        {
            return IsFinite(value.x) && IsFinite(value.y) &&
                   IsFinite(value.z) && IsFinite(value.w) &&
                   Quaternion.Dot(value, value) > 0.000001f;
        }

        private static bool IsValidDirtyRegion(
            UnityEnhancedTerrainDirtyRegion region,
            int width,
            int height)
        {
            return region.X >= 0 && region.Y >= 0 &&
                   region.Width > 0 && region.Height > 0 &&
                   region.X <= width - region.Width &&
                   region.Y <= height - region.Height;
        }

        private GameObject Acquire(ulong stableIdentity)
        {
            GameObject view = pooledEntities.Count > 0 ? pooledEntities.Pop() : new GameObject();
            view.transform.SetParent(worldObjects, false);
            view.name = "EnhancedEntity_" + stableIdentity.ToString("x16");
            view.hideFlags = HideFlags.DontSave;
            return view;
        }

        private void Recycle(GameObject view)
        {
            if (view == null)
            {
                return;
            }

            if (actorPresentation != null) actorPresentation.Release(view);
            view.SetActive(false);
            view.transform.SetParent(worldObjects, false);
            pooledEntities.Push(view);
        }

        private void ClearActiveEntities()
        {
            foreach (GameObject view in activeEntities.Values)
            {
                Recycle(view);
            }

            activeEntities.Clear();
            entityStartPoses.Clear();
            entityTargetPoses.Clear();
        }

        private void DestroyPooledEntities()
        {
            while (pooledEntities.Count > 0)
            {
                GameObject view = pooledEntities.Pop();
                if (view != null)
                {
                    if (Application.isPlaying)
                    {
                        Destroy(view);
                    }
                    else
                    {
                        DestroyImmediate(view);
                    }
                }
            }
        }
    }
}
