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
    public sealed class UnityEnhancedWorldSnapshot
    {
        public ulong Generation;
        public ulong Tick;
        public bool Reset;
        public UnityEnhancedCameraPose CameraPose;
        public UnityEnhancedEntitySnapshot[] Entities = Array.Empty<UnityEnhancedEntitySnapshot>();
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
        private UnityEnhancedDiagnostics diagnostics;
        private ulong generation;
        private ulong lastTick;
        private bool hasSnapshot;

        public UnityEnhancedLifecycleState State { get; private set; }
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
            if (nightLighting != null) nightLighting.ResetForGeneration(0);
            if (vfxPresentation != null) vfxPresentation.ResetForGeneration(0);
            if (actorPresentation != null) actorPresentation.ResetForSession();
            if (qualityAndPost != null) qualityAndPost.ResetForSession();
            if (diagnostics != null) diagnostics.ResetForSession();

            hasSnapshot = false;
            generation = 0;
            lastTick = 0;
            State = UnityEnhancedLifecycleState.Closed;
        }

        /// <summary>
        /// Applies a complete immutable presentation snapshot. Invalid or stale
        /// input is rejected before mutating the current world.
        /// </summary>
        public bool ApplySnapshot(UnityEnhancedWorldSnapshot snapshot)
        {
            if (State != UnityEnhancedLifecycleState.Open || snapshot == null)
            {
                return false;
            }

            if (!ValidateSnapshot(snapshot))
            {
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }

            if (hasSnapshot && snapshot.Generation == generation && snapshot.Tick < lastTick)
            {
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                return false;
            }

            if (!hasSnapshot || snapshot.Reset || snapshot.Generation != generation)
            {
                ResetPresentation(snapshot.Generation);
            }

            if (snapshot.Terrain != null && !terrainPresentation.ApplySnapshot(snapshot.Terrain))
            {
                if (diagnostics != null) diagnostics.RecordRejectedSnapshot();
                State = UnityEnhancedLifecycleState.Faulted;
                return false;
            }

            if (snapshot.Water != null && !waterPresentation.ApplySnapshot(snapshot.Water))
            {
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
                    snapshot.Reset);
            }

            if (snapshot.Quality != null && !qualityAndPost.Apply(snapshot.Quality, cameraRig.PresentationCamera))
            {
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
            State = UnityEnhancedLifecycleState.Open;
            presentationAdapters.Reset(nextGeneration);
            terrainPresentation.ResetPresentation(nextGeneration);
            waterPresentation.ResetPresentation(nextGeneration);
            nightLighting.ResetForGeneration(nextGeneration);
            vfxPresentation.ResetForGeneration(nextGeneration);
            actorPresentation.ResetForSession();
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
            if (snapshot.Entities == null)
            {
                return false;
            }

            seenIdentities.Clear();
            for (int index = 0; index < snapshot.Entities.Length; index++)
            {
                UnityEnhancedEntitySnapshot entity = snapshot.Entities[index];
                if (entity == null || entity.StableIdentity == 0 || entity.Generation != snapshot.Generation)
                {
                    return false;
                }

                if (!seenIdentities.Add(entity.StableIdentity))
                {
                    return false;
                }
            }

            return true;
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
                    Destroy(view);
                }
            }
        }
    }
}
