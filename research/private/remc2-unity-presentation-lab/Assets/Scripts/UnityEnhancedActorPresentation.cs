using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// Stable presentation families.  The values are part of the snapshot ABI
    /// and deliberately describe visuals, not gameplay types.
    /// </summary>
    public enum UnityEnhancedPresentationFamily
    {
        Player = 0,
        Town = 1,
        Goat = 2,
        Villager = 3,
        Archer = 4,
        Firefly = 5,
        Mana = 6,
        Tree = 7,
        Stone = 8,
        Dolmen = 9,
        SpellJar = 10,
        Projectile = 11,
        Impact = 12,
        Fire = 13,
        Smoke = 14,
        Checkpoint = 15,
        Exit = 16,
        Castle = 17,
        Possession = 18,
        Generic = 19
    }

    [DisallowMultipleComponent]
    internal sealed class UnityEnhancedActorVisualState : MonoBehaviour
    {
        public GameObject VisualObject;
        public MeshFilter Filter;
        public MeshRenderer Renderer;
        public MaterialPropertyBlock PropertyBlock;
        public GameObject HaloObject;
        public MeshFilter HaloFilter;
        public MeshRenderer HaloRenderer;
        public GameObject TrailObject;
        public TrailRenderer TrailRenderer;
        public GameObject PlumeObject;
        public MeshFilter PlumeFilter;
        public MeshRenderer PlumeRenderer;
        public GameObject VolumeObject;
        public MeshFilter VolumeFilter;
        public MeshRenderer VolumeRenderer;
        public bool FireballTrailVisible;
        public bool HasFireballTrailPosition;
        public Vector3 LastFireballTrailPosition;
        public bool FireballPlumeVisible;
        public bool HasFireballPlumePosition;
        public Vector3 LastFireballPlumePosition;
        public Vector3 FireballTravelDirection;
        public bool FireballVolumeVisible;
        public int Family = -1;
        public bool FireballProjectileMaterial;

        public void Release()
        {
            if (VisualObject != null)
            {
                VisualObject.SetActive(false);
            }
            if (TrailRenderer != null)
            {
                TrailRenderer.emitting = false;
                TrailRenderer.enabled = false;
                TrailRenderer.Clear();
            }
            if (TrailObject != null)
            {
                TrailObject.SetActive(false);
            }
            if (PlumeObject != null)
            {
                PlumeObject.SetActive(false);
            }
            if (VolumeObject != null)
            {
                VolumeObject.SetActive(false);
            }
            FireballTrailVisible = false;
            HasFireballTrailPosition = false;
            FireballPlumeVisible = false;
            HasFireballPlumePosition = false;
            FireballTravelDirection = Vector3.zero;
            FireballVolumeVisible = false;
        }
    }

    /// <summary>
    /// Supplies pooled presentation views for all Level 1 Night families.
    /// Enhanced uses the qualified owner indexed sprites for the bounded actor
    /// families when the real owner route is configured, while diagnostic-only
    /// snapshots retain the procedural fallback geometry.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedActorPresentation : MonoBehaviour
    {
        private const float BillboardHalfSize = 0.35f;
        // The owner entity Z is authoritative, but Enhanced terrain is
        // bilinearly subdivided between owner height cells. Keep the real
        // indexed sprite just above that interpolated surface so it remains
        // depth-tested without disappearing into a neighboring triangle.
        private const float OwnerActorGroundClearance = 0.12f;
        // Owner pixel rows are authored for the original low-resolution
        // viewport. Preserve their source pixels, but scale the presentation
        // by projected depth so a real owner actor remains readable during the
        // town overflight without changing its source texture or family.
        private const float OwnerActorMinimumReadabilityScale = 1.5f;
        private const float OwnerActorMaximumReadabilityScale = 4f;
        // Target roughly 22 vertical pixels in the 720p owner capture while
        // retaining a hard cap for near-camera actors.
        private const float OwnerActorViewportHeightFraction = 0.03f;
        // Keep distant town silhouettes readable during owner flight without
        // changing their authoritative positions, meshes, or gameplay scale.
        // The depth multiplier starts only beyond the readable near field and
        // is capped so a far structure cannot become a billboard-sized prop.
        private const float TownReadabilityBaseHeight = 2.02f;
        private const float TownReadabilityViewportHeightFraction = 0.075f;
        private const float TownReadabilityMaximumScale = 2.25f;
        // Keep town windows and trim alive at night without turning the
        // generated structure into a pulsing billboard. This is a bounded
        // material treatment; the owner mesh and accent mask remain intact.
        private const float TownAccentShimmerStrength = 0.12f;
        private const float DefaultVerticalFieldOfView = 70f;
        private const float FireballCoreVisualScale = 2.8f;
        // The owner projectile remains the anchor; the halo is deliberately
        // larger so a cast reads as a light event in the world, not as a tiny
        // orange sprite.  This is presentation-only scale, not hitbox scale.
        private const float FireballHaloVisualScale = 9.5f;
        private const float FireballTrailTime = 0.24f;
        private const float FireballTrailStartWidth = 0.38f;
        private const float FireballTrailEndWidth = 0.025f;
        private const float FireballTrailMinimumMovement = 0.035f;
        private const int FireballTrailMaterialKey = -1000;
        private const float FireballPlumeLength = 3.6f;
        private const float FireballPlumeWidth = 1.35f;
        private const float FireballPlumeMinimumMovement = 0.01f;
        private const int FireballPlumeMeshKey = -1001;
        private const int FireballPlumeMaterialKey = -1002;
        // This is a presentation-only light volume: a crossed, tapered veil
        // driven by the authoritative projectile pose. It gives the existing
        // fireball a readable atmospheric body without adding gameplay geometry.
        private const float FireballVolumeLength = 5.8f;
        private const float FireballVolumeWidth = 2.2f;
        private const int FireballVolumeMeshKey = -1003;
        private const int FireballVolumeMaterialKey = -1004;
        private readonly Dictionary<int, Mesh> meshes = new Dictionary<int, Mesh>();
        private readonly Dictionary<int, Material> materials = new Dictionary<int, Material>();
        private readonly HashSet<GameObject> configuredViews = new HashSet<GameObject>();
        private Camera presentationCamera;
        private UnityEnhancedOwnerActorCatalog ownerActorCatalog;
        private bool reducedMotion;
        private bool volumetricEffectsEnabled = true;

        public int ActiveVisualCount { get; private set; }
        public int PooledVisualCount { get; private set; }
        public int LastConfiguredFamily { get; private set; } = -1;
        public int LastSortingOrder { get; private set; }
        public bool VolumetricEffectsEnabled { get { return volumetricEffectsEnabled; } }

        public void SetPresentationCamera(Camera camera)
        {
            presentationCamera = camera;
        }

        public bool ConfigureOwnerActorCache(string cachePath, out string reason)
        {
            if (ownerActorCatalog == null)
            {
                ownerActorCatalog = new UnityEnhancedOwnerActorCatalog();
            }

            return ownerActorCatalog.Configure(cachePath, out reason);
        }

        public void SetReducedMotion(bool value)
        {
            reducedMotion = value;
            foreach (GameObject view in configuredViews)
            {
                UnityEnhancedActorVisualState state = view == null
                    ? null
                    : view.GetComponent<UnityEnhancedActorVisualState>();
                if (state != null && state.PlumeObject != null)
                {
                    state.PlumeObject.SetActive(!value && state.FireballPlumeVisible);
                }
                if (state != null && state.VolumeObject != null)
                {
                    state.VolumeObject.SetActive(
                        !value && volumetricEffectsEnabled && state.FireballVolumeVisible);
                }
            }
        }

        public void SetVolumetricEffectsEnabled(bool value)
        {
            volumetricEffectsEnabled = value;
            foreach (GameObject view in configuredViews)
            {
                UnityEnhancedActorVisualState state = view == null
                    ? null
                    : view.GetComponent<UnityEnhancedActorVisualState>();
                if (state != null && state.VolumeObject != null)
                {
                    state.VolumeObject.SetActive(
                        !reducedMotion && volumetricEffectsEnabled && state.FireballVolumeVisible);
                }
            }
        }

        public static bool HasCoverage(int family)
        {
            return family >= (int)UnityEnhancedPresentationFamily.Player &&
                   family <= (int)UnityEnhancedPresentationFamily.Generic;
        }

        public static int CoverageCount
        {
            get { return (int)UnityEnhancedPresentationFamily.Generic + 1; }
        }

        public void Configure(GameObject view, UnityEnhancedEntitySnapshot entity)
        {
            Configure(
                view,
                entity,
                new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, DefaultVerticalFieldOfView));
        }

        public void Configure(
            GameObject view,
            UnityEnhancedEntitySnapshot entity,
            UnityEnhancedCameraPose cameraPose)
        {
            if (view == null || entity == null || !HasCoverage(entity.PresentationFamily))
            {
                return;
            }

            UnityEnhancedActorVisualState state = view.GetComponent<UnityEnhancedActorVisualState>();
            if (state == null)
            {
                state = view.AddComponent<UnityEnhancedActorVisualState>();
            }

            if (state.VisualObject == null)
            {
                state.VisualObject = new GameObject("EnhancedActorVisual");
                state.VisualObject.transform.SetParent(view.transform, false);
                state.Filter = state.VisualObject.AddComponent<MeshFilter>();
                state.Renderer = state.VisualObject.AddComponent<MeshRenderer>();
                state.VisualObject.hideFlags = HideFlags.DontSave;
            }

            if (ownerActorCatalog != null &&
                IsOwnerActorFamily(entity.PresentationFamily))
            {
                UnityEnhancedOwnerActorSprite ownerSprite;
                if (!ownerActorCatalog.TryResolve(entity, out ownerSprite))
                {
                    // A configured owner route fails closed for qualified
                    // actor families. Never turn a missing owner row back into
                    // one of the generated diagnostic boxes.
                    DisableFireballTrail(state);
                    DisableFireballVolume(state);
                    if (state.HaloObject != null) state.HaloObject.SetActive(false);
                    state.Renderer.enabled = false;
                    state.VisualObject.SetActive(false);
                    state.Family = entity.PresentationFamily;
                    state.FireballProjectileMaterial = false;
                    LastConfiguredFamily = entity.PresentationFamily;
                    LastSortingOrder = entity.RenderLayer;
                    if (configuredViews.Add(view)) ActiveVisualCount++;
                    return;
                }

                DisableFireballTrail(state);
                if (state.HaloObject != null) state.HaloObject.SetActive(false);
                state.Family = entity.PresentationFamily;
                state.FireballProjectileMaterial = false;
                state.Filter.sharedMesh = ownerSprite.Mesh;
                state.Renderer.sharedMaterial = ownerSprite.Material;
                state.Renderer.SetPropertyBlock(null);
                state.Renderer.enabled = true;
                state.Renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                state.Renderer.receiveShadows = false;
                state.VisualObject.SetActive(entity.Visible);
                float ownerScale = OwnerActorReadabilityScale(ownerSprite, entity, cameraPose);
                float ownerHeight = ownerSprite.Height * ownerScale;
                state.VisualObject.transform.localPosition =
                    new Vector3(
                        0f,
                        ownerHeight * 0.5f + OwnerActorGroundClearance,
                        0f);
                state.VisualObject.transform.localScale =
                    new Vector3(
                        ownerSprite.Width * ownerScale,
                        ownerHeight,
                        1f);
                state.VisualObject.transform.rotation = OwnerBillboardRotation();
                state.Renderer.sortingOrder = entity.RenderLayer;
                LastConfiguredFamily = entity.PresentationFamily;
                LastSortingOrder = entity.RenderLayer;
                if (configuredViews.Add(view)) ActiveVisualCount++;
                return;
            }

            bool fireballProjectile = entity.PresentationFamily ==
                                          (int)UnityEnhancedPresentationFamily.Fire &&
                                      IsAuthoritativeProjectileIdentity(entity.StableIdentity);
            bool impactPresentation = entity.PresentationFamily ==
                                          (int)UnityEnhancedPresentationFamily.Impact ||
                                      (entity.PresentationFamily ==
                                           (int)UnityEnhancedPresentationFamily.Fire &&
                                       !fireballProjectile);
            if (!fireballProjectile)
            {
                DisableFireballTrail(state);
                DisableFireballVolume(state);
            }
            if (state.Family != entity.PresentationFamily ||
                state.FireballProjectileMaterial != fireballProjectile)
            {
                state.Family = entity.PresentationFamily;
                state.FireballProjectileMaterial = fireballProjectile;
                state.Filter.sharedMesh = GetMesh(entity.PresentationFamily);
                state.Renderer.sharedMaterial = GetMaterial(
                    entity.PresentationFamily,
                    fireballProjectile,
                    false);
            }

            if (state.PropertyBlock == null)
            {
                state.PropertyBlock = new MaterialPropertyBlock();
            }
            state.PropertyBlock.Clear();
            if (impactPresentation)
            {
                state.PropertyBlock.SetFloat("_ImpactProgress", Mathf.Clamp01(entity.AnimationPhase));
            }
            state.Renderer.SetPropertyBlock(state.PropertyBlock);
            state.Renderer.enabled = true;
            // Views are pooled across owner billboards and generated world
            // structures. Owner sprites deliberately disable shadows, so
            // restore the generated-view contract here before a town, castle,
            // or other mesh is rendered again and reaches the moon shadow map.
            state.Renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.On;
            state.Renderer.receiveShadows = true;

            if (fireballProjectile)
            {
                EnsureFireballHalo(state);
                state.HaloFilter.sharedMesh = GetMesh(entity.PresentationFamily);
                state.HaloRenderer.sharedMaterial = GetMaterial(
                    entity.PresentationFamily,
                    true,
                    true);
                EnsureFireballTrail(state);
                ConfigureFireballTrail(state, entity);
                EnsureFireballPlume(state);
                EnsureFireballVolume(state);
            }
            else if (state.HaloObject != null)
            {
                state.HaloObject.SetActive(false);
            }

            state.VisualObject.SetActive(entity.Visible);
            float baseScale = entity.Scale <= 0f ? 1f : entity.Scale;
            float scale = baseScale;
            float haloScale = baseScale;
            if (entity.PresentationFamily == (int)UnityEnhancedPresentationFamily.Town)
            {
                scale *= StructureScale(entity.StableIdentity) *
                         TownDepthReadabilityScale(entity, cameraPose);
            }
            if (fireballProjectile)
            {
                scale *= FireballCoreVisualScale;
                haloScale *= FireballHaloVisualScale;
            }
            float animationOffset = IsAnimated(entity.PresentationFamily)
                ? Mathf.Sin(entity.AnimationPhase * Mathf.PI * 2f) * 0.05f
                : 0f;
            Vector3 localPosition = new Vector3(
                0f,
                animationOffset - (fireballProjectile ? BillboardHalfSize * scale : 0f),
                0f);
            Quaternion localRotation = Quaternion.Euler(0f, entity.FacingDegrees, 0f);
            state.VisualObject.transform.localPosition = localPosition;
            state.VisualObject.transform.localRotation = localRotation;
            state.VisualObject.transform.localScale = Vector3.one * scale;
            if (fireballProjectile)
            {
                state.HaloObject.SetActive(entity.Visible);
                state.HaloObject.transform.localPosition = new Vector3(
                    0f,
                    animationOffset - BillboardHalfSize * haloScale,
                    0f);
                state.HaloObject.transform.localRotation = localRotation;
                state.HaloObject.transform.localScale = Vector3.one * haloScale;
                ConfigureFireballPlume(state, entity);
                ConfigureFireballVolume(state, entity);
            }
            state.Renderer.sortingOrder = entity.RenderLayer;
            if (state.HaloRenderer != null)
            {
                state.HaloRenderer.sortingOrder = entity.RenderLayer;
            }
            LastConfiguredFamily = entity.PresentationFamily;
            LastSortingOrder = entity.RenderLayer;
            if (configuredViews.Add(view))
            {
                ActiveVisualCount++;
            }
        }

        public void Release(GameObject view)
        {
            if (view == null)
            {
                return;
            }

            UnityEnhancedActorVisualState state = view.GetComponent<UnityEnhancedActorVisualState>();
            if (state != null)
            {
                state.Release();
                if (state.HaloObject != null)
                {
                    state.HaloObject.SetActive(false);
                }
            }

            if (configuredViews.Remove(view))
            {
                ActiveVisualCount = Mathf.Max(0, ActiveVisualCount - 1);
                PooledVisualCount++;
            }
        }

        public void ResetForSession()
        {
            configuredViews.Clear();
            ActiveVisualCount = 0;
            PooledVisualCount = 0;
            LastConfiguredFamily = -1;
            LastSortingOrder = 0;
            reducedMotion = false;
            volumetricEffectsEnabled = true;
        }

        private void OnDestroy()
        {
            if (ownerActorCatalog != null)
            {
                ownerActorCatalog.Dispose();
                ownerActorCatalog = null;
            }

            foreach (Mesh mesh in meshes.Values)
            {
                if (mesh != null)
                {
                    if (Application.isPlaying) Destroy(mesh);
                    else DestroyImmediate(mesh);
                }
            }
            meshes.Clear();
            foreach (Material material in materials.Values)
            {
                if (material != null)
                {
                    if (Application.isPlaying) Destroy(material);
                    else DestroyImmediate(material);
                }
            }
            materials.Clear();
        }

        private Mesh GetMesh(int family)
        {
            Mesh mesh;
            if (meshes.TryGetValue(family, out mesh))
            {
                return mesh;
            }

            UnityEnhancedPresentationFamily familyId = (UnityEnhancedPresentationFamily)family;
            mesh = IsBillboard(familyId) ? CreateBillboardMesh(familyId) : CreateMesh(familyId);
            mesh.name = "EnhancedGeneratedMesh_" + familyId;
            mesh.hideFlags = HideFlags.DontSave;
            meshes.Add(family, mesh);
            return mesh;
        }

        private void EnsureFireballHalo(UnityEnhancedActorVisualState state)
        {
            if (state.HaloObject != null)
            {
                return;
            }

            state.HaloObject = new GameObject("EnhancedFireballHalo");
            state.HaloObject.transform.SetParent(state.VisualObject.transform.parent, false);
            state.HaloObject.hideFlags = HideFlags.DontSave;
            state.HaloFilter = state.HaloObject.AddComponent<MeshFilter>();
            state.HaloRenderer = state.HaloObject.AddComponent<MeshRenderer>();
        }

        private void EnsureFireballTrail(UnityEnhancedActorVisualState state)
        {
            if (state.TrailObject != null)
            {
                return;
            }

            state.TrailObject = new GameObject("EnhancedFireballTrail");
            state.TrailObject.transform.SetParent(state.VisualObject.transform.parent, false);
            state.TrailObject.hideFlags = HideFlags.DontSave;
            state.TrailRenderer = state.TrailObject.AddComponent<TrailRenderer>();
            state.TrailRenderer.material = GetFireballTrailMaterial();
            state.TrailRenderer.time = FireballTrailTime;
            state.TrailRenderer.widthCurve = AnimationCurve.Linear(0f, 1f, 1f, 0.1f);
            state.TrailRenderer.startWidth = FireballTrailStartWidth;
            state.TrailRenderer.endWidth = FireballTrailEndWidth;
            state.TrailRenderer.minVertexDistance = 0.04f;
            state.TrailRenderer.numCornerVertices = 2;
            state.TrailRenderer.numCapVertices = 2;
            state.TrailRenderer.alignment = LineAlignment.View;
            state.TrailRenderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            state.TrailRenderer.receiveShadows = false;
            state.TrailRenderer.colorGradient = FireballTrailGradient();
            state.TrailRenderer.emitting = false;
            state.TrailRenderer.enabled = false;
            state.TrailObject.SetActive(false);
        }

        private static void ConfigureFireballTrail(
            UnityEnhancedActorVisualState state,
            UnityEnhancedEntitySnapshot entity)
        {
            if (!entity.Visible)
            {
                DisableFireballTrail(state);
                return;
            }

            state.TrailObject.SetActive(true);
            state.TrailRenderer.enabled = true;
            // AddPosition receives authoritative world-space entity positions;
            // automatic emission stays off so render-time motion cannot duplicate history.
            state.TrailRenderer.emitting = false;
            if (!state.FireballTrailVisible || !state.HasFireballTrailPosition)
            {
                state.TrailRenderer.Clear();
                state.TrailRenderer.AddPosition(entity.Position);
            }
            else if ((entity.Position - state.LastFireballTrailPosition).sqrMagnitude >=
                     FireballTrailMinimumMovement * FireballTrailMinimumMovement)
            {
                state.TrailRenderer.AddPosition(entity.Position);
            }

            state.LastFireballTrailPosition = entity.Position;
            state.HasFireballTrailPosition = true;
            state.FireballTrailVisible = true;
        }

        private void EnsureFireballPlume(UnityEnhancedActorVisualState state)
        {
            if (state.PlumeObject != null)
            {
                return;
            }

            state.PlumeObject = new GameObject("EnhancedFireballPlume");
            state.PlumeObject.transform.SetParent(state.VisualObject.transform.parent, false);
            state.PlumeObject.hideFlags = HideFlags.DontSave;
            state.PlumeFilter = state.PlumeObject.AddComponent<MeshFilter>();
            state.PlumeRenderer = state.PlumeObject.AddComponent<MeshRenderer>();
            state.PlumeFilter.sharedMesh = GetFireballPlumeMesh();
            state.PlumeRenderer.sharedMaterial = GetFireballPlumeMaterial();
            state.PlumeRenderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            state.PlumeRenderer.receiveShadows = false;
            state.PlumeRenderer.enabled = true;
            state.PlumeObject.SetActive(false);
        }

        private void EnsureFireballVolume(UnityEnhancedActorVisualState state)
        {
            if (state.VolumeObject != null)
            {
                return;
            }

            state.VolumeObject = new GameObject("EnhancedFireballVolume");
            state.VolumeObject.transform.SetParent(state.VisualObject.transform.parent, false);
            state.VolumeObject.hideFlags = HideFlags.DontSave;
            state.VolumeFilter = state.VolumeObject.AddComponent<MeshFilter>();
            state.VolumeRenderer = state.VolumeObject.AddComponent<MeshRenderer>();
            state.VolumeFilter.sharedMesh = GetFireballVolumeMesh();
            state.VolumeRenderer.sharedMaterial = GetFireballVolumeMaterial();
            state.VolumeRenderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            state.VolumeRenderer.receiveShadows = false;
            state.VolumeRenderer.enabled = true;
            state.VolumeObject.SetActive(false);
        }

        private void ConfigureFireballPlume(
            UnityEnhancedActorVisualState state,
            UnityEnhancedEntitySnapshot entity)
        {
            if (!entity.Visible)
            {
                DisableFireballPlume(state);
                return;
            }

            Vector3 direction = ResolveFireballPlumeDirection(state, entity);
            // Match the core's finalized world center. The core billboard uses
            // a family-specific local pivot, so raw entity.Position can be a
            // visible offset from the authoritative projectile center.
            state.PlumeObject.transform.position = state.VisualObject.transform.position;
            state.PlumeObject.transform.rotation = PlumeRotation(direction);
            float baseScale = entity.Scale <= 0f ? 1f : entity.Scale;
            state.PlumeObject.transform.localScale = new Vector3(
                FireballPlumeWidth * baseScale,
                FireballPlumeLength * baseScale,
                baseScale);
            state.PlumeObject.SetActive(!reducedMotion);
            state.FireballPlumeVisible = true;
        }

        private Vector3 ResolveFireballPlumeDirection(
            UnityEnhancedActorVisualState state,
            UnityEnhancedEntitySnapshot entity)
        {
            Vector3 direction = Vector3.zero;
            if (state.HasFireballPlumePosition)
            {
                direction = CameraPlaneDirection(
                    entity.Position - state.LastFireballPlumePosition);
            }

            if (direction.sqrMagnitude < FireballPlumeMinimumMovement * FireballPlumeMinimumMovement)
            {
                direction = CameraPlaneDirection(entity.Rotation * Vector3.forward);
            }

            if (direction.sqrMagnitude < FireballPlumeMinimumMovement * FireballPlumeMinimumMovement)
            {
                direction = presentationCamera == null
                    ? entity.Rotation * Vector3.up
                    : presentationCamera.transform.up;
            }

            state.LastFireballPlumePosition = entity.Position;
            state.HasFireballPlumePosition = true;
            state.FireballTravelDirection = direction.normalized;
            return state.FireballTravelDirection;
        }

        private void ConfigureFireballVolume(
            UnityEnhancedActorVisualState state,
            UnityEnhancedEntitySnapshot entity)
        {
            if (!entity.Visible || reducedMotion || !volumetricEffectsEnabled)
            {
                DisableFireballVolume(state);
                return;
            }

            Vector3 direction = state.FireballTravelDirection;
            if (direction.sqrMagnitude < FireballPlumeMinimumMovement * FireballPlumeMinimumMovement)
            {
                direction = presentationCamera == null
                    ? entity.Rotation * Vector3.forward
                    : presentationCamera.transform.up;
            }

            state.VolumeObject.transform.position = state.VisualObject.transform.position;
            state.VolumeObject.transform.rotation = PlumeRotation(direction.normalized);
            float baseScale = entity.Scale <= 0f ? 1f : entity.Scale;
            state.VolumeObject.transform.localScale = new Vector3(
                FireballVolumeWidth * baseScale,
                FireballVolumeLength * baseScale,
                baseScale);
            state.VolumeObject.SetActive(true);
            state.FireballVolumeVisible = true;
        }

        private Mesh GetFireballVolumeMesh()
        {
            Mesh mesh;
            if (meshes.TryGetValue(FireballVolumeMeshKey, out mesh))
            {
                return mesh;
            }

            mesh = new Mesh
            {
                name = "EnhancedGeneratedMesh_FireballVolume",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                // Camera-facing veil.
                new Vector3(-0.72f, 0f, 0f), new Vector3(0.72f, 0f, 0f),
                new Vector3(-0.52f, -0.40f, 0f), new Vector3(0.52f, -0.40f, 0f),
                new Vector3(-0.12f, -1f, 0f), new Vector3(0.12f, -1f, 0f),
                // Crossed veil keeps the volume readable while the camera
                // and projectile direction change during flight.
                new Vector3(0f, 0f, -0.72f), new Vector3(0f, 0f, 0.72f),
                new Vector3(0f, -0.40f, -0.52f), new Vector3(0f, -0.40f, 0.52f),
                new Vector3(0f, -1f, -0.12f), new Vector3(0f, -1f, 0.12f)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(0f, 1f),
                new Vector2(0.42f, 0f), new Vector2(0.42f, 1f),
                new Vector2(1f, 0f), new Vector2(1f, 1f),
                new Vector2(0f, 0f), new Vector2(0f, 1f),
                new Vector2(0.42f, 0f), new Vector2(0.42f, 1f),
                new Vector2(1f, 0f), new Vector2(1f, 1f)
            };
            mesh.colors = new[]
            {
                new Color(1f, 0.82f, 0.24f, 1f), new Color(1f, 0.40f, 0.04f, 1f),
                new Color(1f, 0.36f, 0.03f, 0.72f), new Color(1f, 0.12f, 0.01f, 0.72f),
                new Color(1f, 0.12f, 0.01f, 0f), new Color(0.30f, 0.01f, 0f, 0f),
                new Color(1f, 0.82f, 0.24f, 1f), new Color(1f, 0.40f, 0.04f, 1f),
                new Color(1f, 0.36f, 0.03f, 0.72f), new Color(1f, 0.12f, 0.01f, 0.72f),
                new Color(1f, 0.12f, 0.01f, 0f), new Color(0.30f, 0.01f, 0f, 0f)
            };
            mesh.triangles = new[]
            {
                0, 2, 1, 1, 2, 3, 2, 4, 3, 3, 4, 5,
                6, 8, 7, 7, 8, 9, 8, 10, 9, 9, 10, 11
            };
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            meshes.Add(FireballVolumeMeshKey, mesh);
            return mesh;
        }

        private Material GetFireballVolumeMaterial()
        {
            Material material;
            if (materials.TryGetValue(FireballVolumeMaterialKey, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/FireballVolume") ??
                            Shader.Find("MagicCarpet/Enhanced/FireballTrail");
            material = shader == null ? null : new Material(shader)
            {
                name = "EnhancedGeneratedMaterial_FireballVolume",
                hideFlags = HideFlags.DontSave
            };
            if (material != null && shader.name == "MagicCarpet/Enhanced/FireballVolume")
            {
                material.enableInstancing = true;
                material.SetColor("_VolumeColor", new Color(1f, 0.10f, 0.012f, 1f));
                material.SetColor("_CoreColor", new Color(1f, 0.72f, 0.16f, 1f));
                material.SetFloat("_Intensity", 2.35f);
                material.SetFloat("_CoreIntensity", 1.15f);
                material.SetFloat("_Opacity", 0.52f);
                material.SetFloat("_ScrollSpeed", 2.8f);
                material.SetFloat("_NoiseScale", 4.5f);
                material.SetFloat("_ShimmerStrength", 0.24f);
            }
            materials.Add(FireballVolumeMaterialKey, material);
            return material;
        }

        private Vector3 CameraPlaneDirection(Vector3 worldDirection)
        {
            if (presentationCamera == null)
            {
                return worldDirection.normalized;
            }

            return Vector3.ProjectOnPlane(
                worldDirection,
                presentationCamera.transform.forward).normalized;
        }

        private Quaternion PlumeRotation(Vector3 direction)
        {
            Vector3 normal = presentationCamera == null
                ? Vector3.forward
                : presentationCamera.transform.forward;
            return Quaternion.LookRotation(normal, direction);
        }

        private static void DisableFireballPlume(UnityEnhancedActorVisualState state)
        {
            if (state == null || state.PlumeObject == null)
            {
                return;
            }

            state.PlumeObject.SetActive(false);
            state.FireballPlumeVisible = false;
            state.HasFireballPlumePosition = false;
        }

        private static void DisableFireballVolume(UnityEnhancedActorVisualState state)
        {
            if (state == null || state.VolumeObject == null)
            {
                return;
            }

            state.VolumeObject.SetActive(false);
            state.FireballVolumeVisible = false;
        }

        private Mesh GetFireballPlumeMesh()
        {
            Mesh mesh;
            if (meshes.TryGetValue(FireballPlumeMeshKey, out mesh))
            {
                return mesh;
            }

            mesh = new Mesh
            {
                name = "EnhancedGeneratedMesh_FireballPlume",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                new Vector3(-0.50f, 0f, 0f), new Vector3(0.50f, 0f, 0f),
                new Vector3(-0.34f, -0.44f, 0f), new Vector3(0.34f, -0.44f, 0f),
                new Vector3(-0.055f, -1f, 0f), new Vector3(0.055f, -1f, 0f)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(0f, 1f),
                new Vector2(0.44f, 0f), new Vector2(0.44f, 1f),
                new Vector2(1f, 0f), new Vector2(1f, 1f)
            };
            mesh.colors = new[]
            {
                new Color(1f, 0.78f, 0.20f, 1f), new Color(1f, 0.18f, 0.025f, 1f),
                new Color(1f, 0.36f, 0.04f, 0.70f), new Color(1f, 0.08f, 0.01f, 0.70f),
                new Color(1f, 0.12f, 0.01f, 0f), new Color(0.35f, 0.01f, 0f, 0f)
            };
            mesh.triangles = new[] { 0, 2, 1, 1, 2, 3, 2, 4, 3, 3, 4, 5 };
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            meshes.Add(FireballPlumeMeshKey, mesh);
            return mesh;
        }

        private Material GetFireballPlumeMaterial()
        {
            Material material;
            if (materials.TryGetValue(FireballPlumeMaterialKey, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/FireballTrail") ??
                            Shader.Find("MagicCarpet/Enhanced/Vfx");
            material = shader == null ? null : new Material(shader)
            {
                name = "EnhancedGeneratedMaterial_FireballPlume",
                hideFlags = HideFlags.DontSave
            };
            if (material != null)
            {
                material.enableInstancing = true;
                if (shader.name == "MagicCarpet/Enhanced/FireballTrail")
                {
                    material.SetColor("_TrailColor", new Color(1f, 0.12f, 0.015f, 1f));
                    material.SetFloat("_Intensity", 5.4f);
                    material.SetFloat("_PulseSpeed", 4.6f);
                    material.SetFloat("_ShimmerStrength", 0.18f);
                    material.SetFloat("_Opacity", 0.96f);
                }
            }
            materials.Add(FireballPlumeMaterialKey, material);
            return material;
        }

        private Material GetFireballTrailMaterial()
        {
            Material material;
            if (materials.TryGetValue(FireballTrailMaterialKey, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/FireballTrail");
            material = shader == null ? null : new Material(shader)
            {
                name = "EnhancedGeneratedMaterial_FireballTrail",
                hideFlags = HideFlags.DontSave
            };
            if (material != null)
            {
                material.enableInstancing = true;
                material.SetColor("_TrailColor", new Color(1f, 0.16f, 0.025f, 1f));
                material.SetFloat("_Intensity", 3.6f);
                material.SetFloat("_PulseSpeed", 3.4f);
                material.SetFloat("_ShimmerStrength", 0.12f);
                material.SetFloat("_Opacity", 0.9f);
            }
            materials.Add(FireballTrailMaterialKey, material);
            return material;
        }

        private static Gradient FireballTrailGradient()
        {
            Gradient gradient = new Gradient();
            gradient.SetKeys(
                new[]
                {
                    new GradientColorKey(new Color(1f, 0.82f, 0.25f, 1f), 0f),
                    new GradientColorKey(new Color(1f, 0.16f, 0.025f, 1f), 0.55f),
                    new GradientColorKey(new Color(0.55f, 0.015f, 0.005f, 1f), 1f)
                },
                new[]
                {
                    new GradientAlphaKey(0.92f, 0f),
                    new GradientAlphaKey(0.48f, 0.55f),
                    new GradientAlphaKey(0f, 1f)
                });
            return gradient;
        }

        private static void DisableFireballTrail(UnityEnhancedActorVisualState state)
        {
            if (state == null || state.TrailRenderer == null)
            {
                return;
            }

            state.TrailRenderer.emitting = false;
            state.TrailRenderer.enabled = false;
            state.TrailRenderer.Clear();
            state.TrailObject.SetActive(false);
            state.FireballTrailVisible = false;
            state.HasFireballTrailPosition = false;
        }

        private Material GetMaterial(int family, bool fireballProjectile, bool halo)
        {
            int materialKey = fireballProjectile
                ? family + (halo ? 3000 : 1000)
                : family;
            Material material;
            if (materials.TryGetValue(materialKey, out material))
            {
                return material;
            }

            UnityEnhancedPresentationFamily familyId = (UnityEnhancedPresentationFamily)family;
            bool fireFamily = familyId == UnityEnhancedPresentationFamily.Fire;
            bool impactPresentation = familyId == UnityEnhancedPresentationFamily.Impact ||
                                      (fireFamily && !fireballProjectile);
            Shader shader = fireballProjectile
                            ? Shader.Find("MagicCarpet/Enhanced/Fireball") ??
                              Shader.Find("MagicCarpet/Enhanced/Vfx")
                            : (impactPresentation
                                ? Shader.Find("MagicCarpet/Enhanced/Impact")
                                : (fireFamily
                                    ? Shader.Find("MagicCarpet/Enhanced/Vfx")
                                    : null)) ??
                            Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
                            Shader.Find("Universal Render Pipeline/Lit") ??
                            Shader.Find("Standard");
            material = shader == null ? null : new Material(shader)
            {
                name = fireballProjectile
                    ? "EnhancedGeneratedMaterial_Fireball"
                    : "EnhancedGeneratedMaterial_" + familyId,
                hideFlags = HideFlags.DontSave,
                // Town and castle meshes already carry a bounded wall, roof,
                // trim, and window palette in their vertex colors. Do not
                // multiply that authored structure palette by one family tint
                // or the night route collapses the silhouette into orange
                // boxes. Creature/VFX families retain their family tint.
                color = IsStructuredFamily(familyId)
                    ? Color.white
                    : FamilyColor(familyId)
            };
            if (material != null)
            {
                material.enableInstancing = true;
                if (fireballProjectile && shader.name == "MagicCarpet/Enhanced/Fireball")
                {
                    material.SetColor("_CoreColor", new Color(1f, 0.72f, 0.22f, 1f));
                    material.SetColor("_HaloColor", new Color(1f, 0.06f, 0.008f, 1f));
                    material.SetFloat("_CoreIntensity", halo ? 0f : 4.8f);
                    material.SetFloat("_HaloIntensity", halo ? 4.6f : 0.75f);
                    material.SetFloat("_CoreRadius", halo ? 0.08f : 0.42f);
                    material.SetFloat("_HaloRadius", halo ? 0.98f : 0.72f);
                    material.SetFloat("_OuterRadius", halo ? 1.56f : 0f);
                    material.SetFloat("_OuterIntensity", halo ? 6.2f : 0f);
                    material.SetFloat("_Opacity", halo ? 0.90f : 0.92f);
                    material.SetFloat("_PulseSpeed", halo ? 3.0f : 2.8f);
                    material.SetFloat("_ShimmerStrength", halo ? 0.24f : 0.1f);
                }
                else if (impactPresentation && shader.name == "MagicCarpet/Enhanced/Impact")
                {
                    material.SetColor("_GlowColor", new Color(1f, 0.18f, 0.025f, 1f));
                    material.SetColor("_ShockwaveColor", new Color(1f, 0.50f, 0.06f, 1f));
                    material.SetFloat("_GlowIntensity", 1.55f);
                    material.SetFloat("_ShockwaveStrength", 2.6f);
                    material.SetFloat("_SparkStrength", 1.25f);
                    material.SetFloat("_Opacity", 0.84f);
                    material.SetFloat("_PulseSpeed", 3.6f);
                    material.SetFloat("_RimStrength", 0.65f);
                }
                else if (fireFamily && shader.name == "MagicCarpet/Enhanced/Vfx")
                {
                    material.SetColor("_GlowColor", new Color(1f, 0.18f, 0.025f, 1f));
                    material.SetFloat("_GlowIntensity", 1.35f);
                    material.SetFloat("_Opacity", 0.78f);
                    material.SetFloat("_PulseSpeed", 2.8f);
                    material.SetFloat("_RimStrength", 1.45f);
                }
                else
                {
                    material.SetFloat("_AmbientBoost", 0.62f);
                    material.SetFloat("_SpecularStrength", 0.12f);
                    material.SetColor("_RimColor", new Color(0.08f, 0.16f, 0.52f, 1f));
                    material.SetFloat("_RimStrength", 0.24f);
                    material.SetColor("_EmissionColor", FamilyColor(familyId));
                    material.SetFloat("_EmissionStrength", EmissionStrength(familyId));
                    material.SetColor("_AccentColor", new Color(1f, 0.20f, 0.025f, 1f));
                    material.SetFloat(
                        "_AccentStrength",
                        familyId == UnityEnhancedPresentationFamily.Town ? 1.10f :
                        familyId == UnityEnhancedPresentationFamily.Castle ? 0.82f : 0f);
                    material.SetFloat(
                        "_AccentShimmerStrength",
                        familyId == UnityEnhancedPresentationFamily.Town
                            ? TownAccentShimmerStrength
                            : 0f);
                }
            }
            materials.Add(materialKey, material);
            return material;
        }

        private static bool IsAuthoritativeProjectileIdentity(ulong stableIdentity)
        {
            return (stableIdentity & 0xf0000000UL) == 0x20000000UL;
        }

        private static bool IsStructuredFamily(UnityEnhancedPresentationFamily family)
        {
            return family == UnityEnhancedPresentationFamily.Town ||
                   family == UnityEnhancedPresentationFamily.Castle;
        }

        private static bool IsOwnerActorFamily(int family)
        {
            return family == (int)UnityEnhancedPresentationFamily.Goat ||
                   family == (int)UnityEnhancedPresentationFamily.Villager ||
                   family == (int)UnityEnhancedPresentationFamily.Dolmen;
        }

        private static float OwnerActorReadabilityScale(
            UnityEnhancedOwnerActorSprite ownerSprite,
            UnityEnhancedEntitySnapshot entity,
            UnityEnhancedCameraPose cameraPose)
        {
            if (ownerSprite == null || !IsFinite(ownerSprite.Height) || ownerSprite.Height <= 0f ||
                entity == null || !IsFinite(entity.Position) || !IsFinite(cameraPose.Position) ||
                !IsFinite(cameraPose.Rotation))
            {
                return OwnerActorMinimumReadabilityScale;
            }

            Vector3 cameraForward = cameraPose.Rotation * Vector3.forward;
            float forwardLength = cameraForward.magnitude;
            if (!IsFinite(cameraForward) || !IsFinite(forwardLength) || forwardLength <= 0.0001f)
            {
                return OwnerActorMinimumReadabilityScale;
            }

            float forwardDepth = Vector3.Dot(
                entity.Position - cameraPose.Position,
                cameraForward / forwardLength);
            if (!IsFinite(forwardDepth) || forwardDepth <= 0f)
            {
                return OwnerActorMinimumReadabilityScale;
            }

            float fieldOfView = cameraPose.FieldOfView;
            if (!IsFinite(fieldOfView) || fieldOfView <= 1f || fieldOfView >= 179f)
            {
                fieldOfView = DefaultVerticalFieldOfView;
            }

            float requiredHeight = 2f * forwardDepth *
                                   Mathf.Tan(fieldOfView * Mathf.Deg2Rad * 0.5f) *
                                   OwnerActorViewportHeightFraction;
            if (!IsFinite(requiredHeight) || requiredHeight <= 0f)
            {
                return OwnerActorMinimumReadabilityScale;
            }

            return Mathf.Clamp(
                requiredHeight / ownerSprite.Height,
                OwnerActorMinimumReadabilityScale,
                OwnerActorMaximumReadabilityScale);
        }

        private static float StructureScale(ulong stableIdentity)
        {
            int variant = (int)((stableIdentity ^ (stableIdentity >> 17)) & 3UL);
            return 0.86f + variant * 0.08f;
        }

        private static float TownDepthReadabilityScale(
            UnityEnhancedEntitySnapshot entity,
            UnityEnhancedCameraPose cameraPose)
        {
            if (entity == null || !IsFinite(entity.Position) ||
                !IsFinite(cameraPose.Position) || !IsFinite(cameraPose.Rotation))
            {
                return 1f;
            }

            Vector3 cameraForward = cameraPose.Rotation * Vector3.forward;
            float forwardLength = cameraForward.magnitude;
            if (!IsFinite(cameraForward) || !IsFinite(forwardLength) ||
                forwardLength <= 0.0001f)
            {
                return 1f;
            }

            float forwardDepth = Vector3.Dot(
                entity.Position - cameraPose.Position,
                cameraForward / forwardLength);
            if (!IsFinite(forwardDepth) || forwardDepth <= 0f)
            {
                return 1f;
            }

            float fieldOfView = cameraPose.FieldOfView;
            if (!IsFinite(fieldOfView) || fieldOfView <= 1f || fieldOfView >= 179f)
            {
                fieldOfView = DefaultVerticalFieldOfView;
            }

            float requiredHeight = 2f * forwardDepth *
                                   Mathf.Tan(fieldOfView * Mathf.Deg2Rad * 0.5f) *
                                   TownReadabilityViewportHeightFraction;
            if (!IsFinite(requiredHeight) || requiredHeight <= 0f)
            {
                return 1f;
            }

            return Mathf.Clamp(
                Mathf.Max(1f, requiredHeight / TownReadabilityBaseHeight),
                1f,
                TownReadabilityMaximumScale);
        }

        private static bool IsFinite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        private static bool IsFinite(Vector3 value)
        {
            return IsFinite(value.x) && IsFinite(value.y) && IsFinite(value.z);
        }

        private static bool IsFinite(Quaternion value)
        {
            return IsFinite(value.x) && IsFinite(value.y) &&
                   IsFinite(value.z) && IsFinite(value.w);
        }

        private Quaternion OwnerBillboardRotation()
        {
            if (presentationCamera == null)
            {
                return Quaternion.identity;
            }

            return Quaternion.LookRotation(
                -presentationCamera.transform.forward,
                presentationCamera.transform.up);
        }

        private static bool IsBillboard(UnityEnhancedPresentationFamily family)
        {
            return family == UnityEnhancedPresentationFamily.Firefly ||
                   family == UnityEnhancedPresentationFamily.Mana ||
                   family == UnityEnhancedPresentationFamily.Projectile ||
                   family == UnityEnhancedPresentationFamily.Impact ||
                   family == UnityEnhancedPresentationFamily.Fire ||
                   family == UnityEnhancedPresentationFamily.Smoke ||
                   family == UnityEnhancedPresentationFamily.SpellJar;
        }

        private static bool IsAnimated(int family)
        {
            return family == (int)UnityEnhancedPresentationFamily.Player ||
                   family == (int)UnityEnhancedPresentationFamily.Goat ||
                   family == (int)UnityEnhancedPresentationFamily.Villager ||
                   family == (int)UnityEnhancedPresentationFamily.Archer ||
                   family == (int)UnityEnhancedPresentationFamily.Firefly ||
                   family == (int)UnityEnhancedPresentationFamily.Mana;
        }

        private static Mesh CreateBillboardMesh(UnityEnhancedPresentationFamily family)
        {
            float half = family == UnityEnhancedPresentationFamily.Smoke
                ? 0.7f
                : BillboardHalfSize;
            Mesh mesh = new Mesh();
            mesh.vertices = new[]
            {
                new Vector3(-half, 0f, 0f), new Vector3(half, 0f, 0f),
                new Vector3(half, half * 2f, 0f), new Vector3(-half, half * 2f, 0f),
                new Vector3(0f, 0f, -half), new Vector3(0f, half * 2f, -half),
                new Vector3(0f, 0f, half), new Vector3(0f, half * 2f, half)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(1f, 0f), new Vector2(1f, 1f), new Vector2(0f, 1f),
                new Vector2(0f, 0f), new Vector2(0f, 1f), new Vector2(1f, 0f), new Vector2(1f, 1f)
            };
            mesh.triangles = new[]
            {
                0, 2, 1, 0, 3, 2,
                4, 5, 7, 4, 7, 6
            };
            mesh.RecalculateNormals();
            return mesh;
        }

        private static Mesh CreateMesh(UnityEnhancedPresentationFamily family)
        {
            switch (family)
            {
                case UnityEnhancedPresentationFamily.Town:
                    return CreateTownMesh();
                case UnityEnhancedPresentationFamily.Castle:
                    return CreateCastleMesh();
                case UnityEnhancedPresentationFamily.Tree:
                    return CreateTreeMesh();
                case UnityEnhancedPresentationFamily.Stone:
                case UnityEnhancedPresentationFamily.Dolmen:
                    return CreateRockMesh(family);
                case UnityEnhancedPresentationFamily.Player:
                case UnityEnhancedPresentationFamily.Goat:
                case UnityEnhancedPresentationFamily.Villager:
                case UnityEnhancedPresentationFamily.Archer:
                case UnityEnhancedPresentationFamily.Possession:
                case UnityEnhancedPresentationFamily.Generic:
                    return CreateCreatureMesh(family);
                default:
                    return CreateBoxMesh(family);
            }
        }

        private static Mesh CreateTownMesh()
        {
            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            Color wall = new Color(0.96f, 0.72f, 0.48f, 1f);
            Color roof = new Color(0.72f, 0.34f, 0.20f, 1f);
            Color trim = new Color(1f, 0.86f, 0.54f, 1f);
            Color window = new Color(1f, 0.96f, 0.48f, 1f);

            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.05f, 0f, -0.75f),
                new Vector3(1.05f, 1.45f, 0.75f),
                wall);
            AppendPyramid(
                vertices,
                triangles,
                colors,
                new Vector3(-1.18f, 1.45f, -0.86f),
                new Vector3(1.18f, 1.45f, 0.86f),
                0.85f,
                roof);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.22f, 0f, -0.79f),
                new Vector3(0.22f, 0.72f, -0.70f),
                trim);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.68f, 0.65f, -0.80f),
                new Vector3(-0.38f, 0.98f, -0.70f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.38f, 0.65f, -0.80f),
                new Vector3(0.68f, 0.98f, -0.70f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.86f, 0.75f, 0.70f),
                new Vector3(-0.52f, 1.05f, 0.80f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.52f, 0.75f, 0.70f),
                new Vector3(0.86f, 1.05f, 0.80f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.16f, -0.10f, -0.88f),
                new Vector3(1.16f, 0.16f, 0.88f),
                new Color(0.28f, 0.34f, 0.46f, 1f));
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.25f, 1.34f, -0.92f),
                new Vector3(1.25f, 1.53f, 0.92f),
                new Color(0.48f, 0.22f, 0.16f, 1f));
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.72f, 1.44f, 0.22f),
                new Vector3(-0.42f, 2.02f, 0.48f),
                new Color(0.44f, 0.27f, 0.23f, 1f));
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.42f, 1.44f, 0.22f),
                new Vector3(0.72f, 2.02f, 0.48f),
                new Color(0.44f, 0.27f, 0.23f, 1f));
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.78f, 1.28f, -0.12f),
                new Vector3(-0.48f, 1.46f, 0.12f),
                roof);
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh CreateCastleMesh()
        {
            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            Color stone = new Color(0.78f, 0.84f, 0.98f, 1f);
            Color tower = new Color(0.62f, 0.70f, 0.92f, 1f);
            Color roof = new Color(0.42f, 0.34f, 0.70f, 1f);
            Color window = new Color(1f, 0.90f, 0.42f, 1f);

            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.75f, 0f, -0.62f),
                new Vector3(0.75f, 2.15f, 0.62f),
                stone);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.35f, 0f, -0.78f),
                new Vector3(-0.72f, 2.75f, -0.15f),
                tower);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.72f, 0f, -0.78f),
                new Vector3(1.35f, 2.75f, -0.15f),
                tower);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.35f, 0f, 0.15f),
                new Vector3(-0.72f, 2.55f, 0.78f),
                tower);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.72f, 0f, 0.15f),
                new Vector3(1.35f, 2.55f, 0.78f),
                tower);
            AppendPyramid(vertices, triangles, colors,
                new Vector3(-1.46f, 2.75f, -0.88f),
                new Vector3(-0.61f, 2.75f, 0.02f), 0.62f, roof);
            AppendPyramid(vertices, triangles, colors,
                new Vector3(0.61f, 2.75f, -0.88f),
                new Vector3(1.46f, 2.75f, 0.02f), 0.62f, roof);
            AppendPyramid(vertices, triangles, colors,
                new Vector3(-1.46f, 2.55f, 0.02f),
                new Vector3(-0.61f, 2.55f, 0.88f), 0.58f, roof);
            AppendPyramid(vertices, triangles, colors,
                new Vector3(0.61f, 2.55f, 0.02f),
                new Vector3(1.46f, 2.55f, 0.88f), 0.58f, roof);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-0.30f, 0.05f, -0.84f),
                new Vector3(0.30f, 1.05f, -0.68f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(-1.08f, 1.10f, -0.84f),
                new Vector3(-0.88f, 1.48f, -0.70f),
                window);
            AppendBox(
                vertices,
                triangles,
                colors,
                new Vector3(0.88f, 1.10f, -0.84f),
                new Vector3(1.08f, 1.48f, -0.70f),
                window);
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh CreateTreeMesh()
        {
            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            AppendBox(vertices, triangles, colors,
                new Vector3(-0.20f, 0f, -0.20f),
                new Vector3(0.20f, 1.45f, 0.20f),
                new Color(0.72f, 0.48f, 0.28f, 1f));
            AppendOctahedron(vertices, triangles, colors,
                new Vector3(0f, 2.05f, 0f), 1.05f,
                new Color(0.48f, 0.86f, 0.38f, 1f));
            AppendOctahedron(vertices, triangles, colors,
                new Vector3(-0.35f, 2.75f, 0.12f), 0.72f,
                new Color(0.36f, 0.72f, 0.34f, 1f));
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh CreateRockMesh(UnityEnhancedPresentationFamily family)
        {
            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            float radius = family == UnityEnhancedPresentationFamily.Dolmen ? 0.9f : 0.65f;
            float height = family == UnityEnhancedPresentationFamily.Dolmen ? 1.75f : 1.15f;
            AppendOctahedron(vertices, triangles, colors,
                new Vector3(0f, height * 0.55f, 0f), radius,
                family == UnityEnhancedPresentationFamily.Dolmen
                    ? new Color(0.70f, 0.74f, 0.88f, 1f)
                    : new Color(0.82f, 0.86f, 0.90f, 1f));
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh CreateCreatureMesh(UnityEnhancedPresentationFamily family)
        {
            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            float width = family == UnityEnhancedPresentationFamily.Player ? 0.34f : 0.27f;
            float bodyHeight = family == UnityEnhancedPresentationFamily.Player ? 0.95f : 0.72f;
            Color body = new Color(0.94f, 0.96f, 1f, 1f);
            Color head = new Color(1f, 0.88f, 0.70f, 1f);
            AppendBox(vertices, triangles, colors,
                new Vector3(-width, 0.05f, -width * 0.60f),
                new Vector3(width, bodyHeight, width * 0.60f),
                body);
            AppendOctahedron(vertices, triangles, colors,
                new Vector3(0f, bodyHeight + 0.28f, 0f), width * 0.78f, head);
            if (family == UnityEnhancedPresentationFamily.Archer)
            {
                AppendBox(vertices, triangles, colors,
                    new Vector3(width * 0.75f, 0.40f, -0.06f),
                    new Vector3(width * 1.65f, 0.48f, 0.06f),
                    new Color(0.64f, 0.42f, 0.20f, 1f));
            }
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh CreateBoxMesh(UnityEnhancedPresentationFamily family)
        {
            Vector3 halfExtents;
            switch (family)
            {
                case UnityEnhancedPresentationFamily.Tree:
                    halfExtents = new Vector3(0.35f, 1.25f, 0.35f);
                    break;
                case UnityEnhancedPresentationFamily.Castle:
                case UnityEnhancedPresentationFamily.Town:
                    halfExtents = new Vector3(0.9f, 0.8f, 0.9f);
                    break;
                case UnityEnhancedPresentationFamily.Dolmen:
                    halfExtents = new Vector3(0.7f, 1.1f, 0.5f);
                    break;
                default:
                    halfExtents = new Vector3(0.35f, 0.55f, 0.35f);
                    break;
            }

            var vertices = new List<Vector3>();
            var triangles = new List<int>();
            var colors = new List<Color>();
            AppendBox(vertices, triangles, colors,
                new Vector3(-halfExtents.x, 0f, -halfExtents.z),
                new Vector3(halfExtents.x, halfExtents.y * 2f, halfExtents.z),
                Color.white);
            return BuildMesh(vertices, triangles, colors);
        }

        private static Mesh BuildMesh(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors)
        {
            Mesh mesh = new Mesh();
            mesh.SetVertices(vertices);
            mesh.SetTriangles(triangles, 0);
            mesh.SetColors(colors);
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private static void AppendBox(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors,
            Vector3 min,
            Vector3 max,
            Color color)
        {
            AppendQuad(vertices, triangles, colors,
                new Vector3(min.x, min.y, min.z),
                new Vector3(min.x, max.y, min.z),
                new Vector3(max.x, max.y, min.z),
                new Vector3(max.x, min.y, min.z), color);
            AppendQuad(vertices, triangles, colors,
                new Vector3(min.x, min.y, max.z),
                new Vector3(max.x, min.y, max.z),
                new Vector3(max.x, max.y, max.z),
                new Vector3(min.x, max.y, max.z), color);
            AppendQuad(vertices, triangles, colors,
                new Vector3(min.x, min.y, min.z),
                new Vector3(min.x, min.y, max.z),
                new Vector3(min.x, max.y, max.z),
                new Vector3(min.x, max.y, min.z), color);
            AppendQuad(vertices, triangles, colors,
                new Vector3(max.x, min.y, min.z),
                new Vector3(max.x, max.y, min.z),
                new Vector3(max.x, max.y, max.z),
                new Vector3(max.x, min.y, max.z), color);
            AppendQuad(vertices, triangles, colors,
                new Vector3(min.x, max.y, min.z),
                new Vector3(min.x, max.y, max.z),
                new Vector3(max.x, max.y, max.z),
                new Vector3(max.x, max.y, min.z), color);
            AppendQuad(vertices, triangles, colors,
                new Vector3(min.x, min.y, min.z),
                new Vector3(max.x, min.y, min.z),
                new Vector3(max.x, min.y, max.z),
                new Vector3(min.x, min.y, max.z), color);
        }

        private static void AppendPyramid(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors,
            Vector3 min,
            Vector3 max,
            float height,
            Color color)
        {
            Vector3 p0 = new Vector3(min.x, min.y, min.z);
            Vector3 p1 = new Vector3(max.x, min.y, min.z);
            Vector3 p2 = new Vector3(max.x, min.y, max.z);
            Vector3 p3 = new Vector3(min.x, min.y, max.z);
            Vector3 apex = new Vector3((min.x + max.x) * 0.5f, min.y + height, (min.z + max.z) * 0.5f);
            AppendTriangle(vertices, triangles, colors, p1, p0, apex, color);
            AppendTriangle(vertices, triangles, colors, p3, p2, apex, color);
            AppendTriangle(vertices, triangles, colors, p0, p3, apex, color);
            AppendTriangle(vertices, triangles, colors, p2, p1, apex, color);
        }

        private static void AppendOctahedron(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors,
            Vector3 center,
            float radius,
            Color color)
        {
            Vector3 top = center + Vector3.up * radius;
            Vector3 bottom = center - Vector3.up * radius;
            Vector3 left = center + Vector3.left * radius;
            Vector3 right = center + Vector3.right * radius;
            Vector3 front = center + Vector3.forward * radius;
            Vector3 back = center + Vector3.back * radius;
            AppendTriangle(vertices, triangles, colors, top, front, right, color);
            AppendTriangle(vertices, triangles, colors, top, right, back, color);
            AppendTriangle(vertices, triangles, colors, top, back, left, color);
            AppendTriangle(vertices, triangles, colors, top, left, front, color);
            AppendTriangle(vertices, triangles, colors, bottom, right, front, color);
            AppendTriangle(vertices, triangles, colors, bottom, back, right, color);
            AppendTriangle(vertices, triangles, colors, bottom, left, back, color);
            AppendTriangle(vertices, triangles, colors, bottom, front, left, color);
        }

        private static void AppendQuad(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors,
            Vector3 a,
            Vector3 b,
            Vector3 c,
            Vector3 d,
            Color color)
        {
            int start = vertices.Count;
            vertices.Add(a);
            vertices.Add(b);
            vertices.Add(c);
            vertices.Add(d);
            colors.Add(color);
            colors.Add(color);
            colors.Add(color);
            colors.Add(color);
            triangles.Add(start);
            triangles.Add(start + 1);
            triangles.Add(start + 2);
            triangles.Add(start);
            triangles.Add(start + 2);
            triangles.Add(start + 3);
        }

        private static void AppendTriangle(
            List<Vector3> vertices,
            List<int> triangles,
            List<Color> colors,
            Vector3 a,
            Vector3 b,
            Vector3 c,
            Color color)
        {
            int start = vertices.Count;
            vertices.Add(a);
            vertices.Add(b);
            vertices.Add(c);
            colors.Add(color);
            colors.Add(color);
            colors.Add(color);
            triangles.Add(start);
            triangles.Add(start + 1);
            triangles.Add(start + 2);
        }

        private static Color FamilyColor(UnityEnhancedPresentationFamily family)
        {
            switch (family)
            {
                case UnityEnhancedPresentationFamily.Player: return new Color(0.22f, 0.74f, 1f);
                case UnityEnhancedPresentationFamily.Town: return new Color(0.82f, 0.48f, 0.23f);
                case UnityEnhancedPresentationFamily.Goat: return new Color(0.82f, 0.78f, 0.66f);
                case UnityEnhancedPresentationFamily.Villager: return new Color(0.76f, 0.58f, 0.32f);
                case UnityEnhancedPresentationFamily.Archer: return new Color(0.48f, 0.72f, 0.34f);
                case UnityEnhancedPresentationFamily.Firefly: return new Color(0.72f, 1f, 0.3f);
                case UnityEnhancedPresentationFamily.Mana: return new Color(0.3f, 0.8f, 1f);
                case UnityEnhancedPresentationFamily.Tree: return new Color(0.2f, 0.42f, 0.18f);
                case UnityEnhancedPresentationFamily.Stone: return new Color(0.42f, 0.44f, 0.48f);
                case UnityEnhancedPresentationFamily.Dolmen: return new Color(0.36f, 0.38f, 0.44f);
                case UnityEnhancedPresentationFamily.SpellJar: return new Color(0.7f, 0.26f, 0.84f);
                case UnityEnhancedPresentationFamily.Projectile: return new Color(1f, 0.5f, 0.1f);
                case UnityEnhancedPresentationFamily.Impact: return new Color(1f, 0.85f, 0.25f);
                case UnityEnhancedPresentationFamily.Fire: return new Color(1f, 0.24f, 0.05f);
                case UnityEnhancedPresentationFamily.Smoke: return new Color(0.3f, 0.32f, 0.4f);
                case UnityEnhancedPresentationFamily.Checkpoint: return new Color(0.3f, 0.8f, 0.68f);
                case UnityEnhancedPresentationFamily.Exit: return new Color(0.78f, 0.2f, 0.7f);
                case UnityEnhancedPresentationFamily.Castle: return new Color(0.68f, 0.52f, 0.38f);
                case UnityEnhancedPresentationFamily.Possession: return new Color(0.84f, 0.2f, 0.36f);
                default: return Color.white;
            }
        }

        private static float EmissionStrength(UnityEnhancedPresentationFamily family)
        {
            switch (family)
            {
                case UnityEnhancedPresentationFamily.Town:
                case UnityEnhancedPresentationFamily.Castle:
                    return 0.14f;
                case UnityEnhancedPresentationFamily.Firefly:
                    return 0.28f;
                case UnityEnhancedPresentationFamily.Mana:
                    return 0.32f;
                case UnityEnhancedPresentationFamily.SpellJar:
                case UnityEnhancedPresentationFamily.Checkpoint:
                case UnityEnhancedPresentationFamily.Exit:
                    return 0.22f;
                case UnityEnhancedPresentationFamily.Projectile:
                case UnityEnhancedPresentationFamily.Impact:
                case UnityEnhancedPresentationFamily.Fire:
                    return 0.30f;
                default:
                    return 0f;
            }
        }

    }
}
