using System;
using System.Collections.Generic;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    public enum UnityEnhancedVfxKind
    {
        SpellCast = 0,
        ManaPickup = 1,
        Destruction = 2,
        Weather = 3,
        SpeedTrail = 4,
        Impact = 5,
        Fire = 6,
        Smoke = 7
    }

    [Serializable]
    public struct UnityEnhancedVfxEvent
    {
        public ulong Sequence;
        public ulong Generation;
        public ulong Tick;
        public ulong StableIdentity;
        public UnityEnhancedVfxKind Kind;
        public Vector3 Position;
        public float Intensity;
        public uint DurationTicks;

        public UnityEnhancedVfxEvent(
            ulong sequence,
            ulong generation,
            ulong tick,
            ulong stableIdentity,
            UnityEnhancedVfxKind kind,
            Vector3 position,
            float intensity,
            uint durationTicks)
        {
            Sequence = sequence;
            Generation = generation;
            Tick = tick;
            StableIdentity = stableIdentity;
            Kind = kind;
            Position = position;
            Intensity = intensity;
            DurationTicks = durationTicks;
        }
    }

    internal sealed class UnityEnhancedVfxVisual
    {
        public GameObject Object;
        public MeshFilter Filter;
        public MeshRenderer Renderer;
        public ulong Sequence;
        public ulong ExpireTick;
    }

    /// <summary>
    /// Bounded event-driven effects.  Effects are presentation-only and are
    /// replayable from the same ordered event stream without changing gameplay.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedVfxPresentation : MonoBehaviour
    {
        private const float FireballEventVisualScale = 1.8f;
        // Owner impacts can occur at the far edge of the flight camera. Keep
        // the authoritative position, but give the transient additive cue a
        // fixed readability scale comparable to the existing fireball halo.
        private const float ImpactEventVisualScale = 5.0f;
        private const int SpeedReadabilityBaseCount = 8;
        private const int SpeedReadabilityMaximumCount = 12;
        private const int SpeedReadabilityAuthoritativeCountCap = 4;
        private const float SpeedReadabilityStart = 48f;
        private const float SpeedReadabilityFull = 80f;
        private const float SpeedReadabilityDepth = 5f;
        private const float SpeedReadabilityMinimumRadius = 0.38f;
        private const float SpeedReadabilityMaximumRadius = 0.58f;
        private const float SpeedReadabilityMinimumLength = 0.06f;
        private const float SpeedReadabilityMaximumLength = 0.14f;
        private const float SpeedReadabilityMinimumWidth = 0.010f;
        private const float SpeedReadabilityMaximumWidth = 0.018f;
        private readonly Stack<UnityEnhancedVfxVisual> pool = new Stack<UnityEnhancedVfxVisual>();
        private readonly Dictionary<ulong, UnityEnhancedVfxVisual> active = new Dictionary<ulong, UnityEnhancedVfxVisual>();
        private readonly Dictionary<UnityEnhancedVfxKind, Mesh> meshes = new Dictionary<UnityEnhancedVfxKind, Mesh>();
        private readonly Dictionary<UnityEnhancedVfxKind, Material> materials = new Dictionary<UnityEnhancedVfxKind, Material>();
        private readonly List<UnityEnhancedVfxVisual> speedReadabilityPool = new List<UnityEnhancedVfxVisual>();
        private Transform effectRoot;
        private Transform speedReadabilityRoot;
        private Camera presentationCamera;
        private ulong generation;
        private ulong lastEventSequence;
        private ulong lastTick;
        private bool reducedMotion;

        public int MaxActiveEffects { get; set; } = 128;
        public int ActiveEffectCount { get { return active.Count; } }
        public int PooledEffectCount { get { return pool.Count; } }
        public int DroppedEffectCount { get; private set; }
        public int DuplicateEffectCount { get; private set; }
        public int ActiveSpeedStreakCount { get; private set; }
        public int SpeedStreakPoolCount { get { return speedReadabilityPool.Count; } }
        public ulong Generation { get { return generation; } }
        public ulong LastEventSequence { get { return lastEventSequence; } }
        public bool ReducedMotion { get { return reducedMotion; } }
        public bool CameraComfortEnabled { get; private set; } = true;

        public void SetPresentationCamera(Camera camera)
        {
            presentationCamera = camera;
        }

        public void SetCameraComfort(bool enabled)
        {
            CameraComfortEnabled = enabled;
        }

        public static bool HasCoverage(UnityEnhancedVfxKind kind)
        {
            return kind >= UnityEnhancedVfxKind.SpellCast && kind <= UnityEnhancedVfxKind.Smoke;
        }

        public void SetReducedMotion(bool value)
        {
            reducedMotion = value;
            if (value)
            {
                ClearSpeedReadability();
            }
            foreach (UnityEnhancedVfxVisual visual in active.Values)
            {
                if (visual.Object != null)
                {
                    visual.Object.transform.localScale = Vector3.one * (value ? 0.6f : 1f);
                }
            }
        }

        public void ResetForGeneration(ulong nextGeneration)
        {
            generation = nextGeneration;
            lastEventSequence = 0;
            lastTick = 0;
            DroppedEffectCount = 0;
            DuplicateEffectCount = 0;
            RecycleAll();
            ClearSpeedReadability();
        }

        public bool ApplyEvent(UnityEnhancedVfxEvent vfxEvent)
        {
            if (!HasCoverage(vfxEvent.Kind) || vfxEvent.Generation != generation || vfxEvent.Sequence == 0)
            {
                return false;
            }

            if (vfxEvent.Sequence <= lastEventSequence)
            {
                DuplicateEffectCount++;
                return false;
            }

            lastEventSequence = vfxEvent.Sequence;
            if (active.Count >= Mathf.Max(1, MaxActiveEffects))
            {
                DroppedEffectCount++;
                return false;
            }

            EnsureHierarchy();
            UnityEnhancedVfxVisual visual = pool.Count > 0 ? pool.Pop() : CreateVisual();
            visual.Sequence = vfxEvent.Sequence;
            ulong duration = vfxEvent.DurationTicks == 0 ? 1UL : vfxEvent.DurationTicks;
            visual.ExpireTick = vfxEvent.Tick + duration;
            visual.Object.transform.position = vfxEvent.Position;
            visual.Object.transform.rotation = vfxEvent.Kind == UnityEnhancedVfxKind.Impact &&
                                               presentationCamera != null
                ? Quaternion.LookRotation(
                    -presentationCamera.transform.forward,
                    presentationCamera.transform.up)
                : Quaternion.identity;
            float intensity = Mathf.Clamp(vfxEvent.Intensity <= 0f ? 1f : vfxEvent.Intensity, 0.1f, 4f);
            float eventScale = vfxEvent.Kind == UnityEnhancedVfxKind.Fire
                ? FireballEventVisualScale
                : vfxEvent.Kind == UnityEnhancedVfxKind.Impact
                    ? ImpactEventVisualScale
                : 1f;
            visual.Object.transform.localScale = Vector3.one * intensity * eventScale *
                                                  (reducedMotion ? 0.6f : 1f);
            visual.Filter.sharedMesh = GetMesh(vfxEvent.Kind);
            visual.Renderer.sharedMaterial = GetMaterial(vfxEvent.Kind);
            visual.Object.SetActive(true);
            active.Add(vfxEvent.Sequence, visual);
            return true;
        }

        /// <summary>
        /// Consumes owner HUD speed state as a camera-local readability cue.
        /// This is deliberately separate from the ordered VFX event stream:
        /// speed is continuous mission state, not a synthetic Unity event.
        /// </summary>
        public void ApplySpeedReadability(
            Camera camera,
            ulong tick,
            int actualSpeed,
            byte authoritativeTrailCount,
            bool enabled)
        {
            if (!enabled || reducedMotion || camera == null)
            {
                ClearSpeedReadability();
                return;
            }

            float speed = Mathf.Abs((float)actualSpeed);
            if (speed <= SpeedReadabilityStart)
            {
                ClearSpeedReadability();
                return;
            }

            float speedBlend = Mathf.InverseLerp(
                SpeedReadabilityStart,
                SpeedReadabilityFull,
                speed);
            int speedDrivenCount = Mathf.Clamp(
                Mathf.CeilToInt(speedBlend * SpeedReadabilityBaseCount),
                0,
                SpeedReadabilityBaseCount);
            int ownerDrivenCount = Mathf.Clamp(
                (int)authoritativeTrailCount,
                0,
                SpeedReadabilityAuthoritativeCountCap);
            int targetCount = Mathf.Clamp(
                speedDrivenCount + ownerDrivenCount,
                0,
                SpeedReadabilityMaximumCount);
            if (targetCount == 0)
            {
                ClearSpeedReadability();
                return;
            }

            EnsureSpeedReadabilityHierarchy(camera);
            for (int index = 0; index < targetCount; index++)
            {
                UnityEnhancedVfxVisual visual = EnsureSpeedReadabilityVisual(index);
                ConfigureSpeedReadabilityVisual(
                    visual,
                    camera,
                    tick,
                    index,
                    speedBlend);
                visual.Object.SetActive(true);
            }

            for (int index = targetCount; index < speedReadabilityPool.Count; index++)
            {
                speedReadabilityPool[index].Object.SetActive(false);
            }
            ActiveSpeedStreakCount = targetCount;
        }

        public void ClearSpeedReadability()
        {
            for (int index = 0; index < speedReadabilityPool.Count; index++)
            {
                UnityEnhancedVfxVisual visual = speedReadabilityPool[index];
                if (visual.Object != null)
                {
                    visual.Object.SetActive(false);
                }
            }
            ActiveSpeedStreakCount = 0;
        }

        public int ApplyReplay(IList<UnityEnhancedVfxEvent> events, ulong replayGeneration)
        {
            ResetForGeneration(replayGeneration);
            int accepted = 0;
            if (events == null)
            {
                return 0;
            }

            for (int index = 0; index < events.Count; index++)
            {
                if (ApplyEvent(events[index]))
                {
                    accepted++;
                }
            }
            return accepted;
        }

        public void Advance(ulong tick)
        {
            lastTick = tick;
            List<ulong> expired = null;
            foreach (KeyValuePair<ulong, UnityEnhancedVfxVisual> pair in active)
            {
                if (pair.Value.ExpireTick <= tick)
                {
                    if (expired == null) expired = new List<ulong>();
                    expired.Add(pair.Key);
                }
            }

            if (expired == null)
            {
                return;
            }

            for (int index = 0; index < expired.Count; index++)
            {
                UnityEnhancedVfxVisual visual = active[expired[index]];
                active.Remove(expired[index]);
                visual.Object.SetActive(false);
                pool.Push(visual);
            }
        }

        private void EnsureHierarchy()
        {
            if (effectRoot != null)
            {
                return;
            }

            GameObject root = new GameObject("EnhancedVfxPool");
            root.transform.SetParent(transform, false);
            root.hideFlags = HideFlags.DontSave;
            effectRoot = root.transform;
        }

        private UnityEnhancedVfxVisual CreateVisual()
        {
            GameObject effect = new GameObject("EnhancedVfx");
            effect.transform.SetParent(effectRoot, false);
            effect.hideFlags = HideFlags.DontSave;
            MeshFilter filter = effect.AddComponent<MeshFilter>();
            MeshRenderer renderer = effect.AddComponent<MeshRenderer>();
            return new UnityEnhancedVfxVisual { Object = effect, Filter = filter, Renderer = renderer };
        }

        private void EnsureSpeedReadabilityHierarchy(Camera camera)
        {
            if (speedReadabilityRoot == null)
            {
                GameObject root = new GameObject("EnhancedSpeedStreaks");
                root.hideFlags = HideFlags.DontSave;
                speedReadabilityRoot = root.transform;
            }

            if (speedReadabilityRoot.parent != camera.transform)
            {
                speedReadabilityRoot.SetParent(camera.transform, false);
            }
        }

        private UnityEnhancedVfxVisual EnsureSpeedReadabilityVisual(int index)
        {
            while (speedReadabilityPool.Count <= index)
            {
                GameObject streak = new GameObject(
                    "EnhancedSpeedStreak_" + speedReadabilityPool.Count);
                streak.transform.SetParent(speedReadabilityRoot, false);
                streak.hideFlags = HideFlags.DontSave;
                MeshFilter filter = streak.AddComponent<MeshFilter>();
                MeshRenderer renderer = streak.AddComponent<MeshRenderer>();
                renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                renderer.receiveShadows = false;
                speedReadabilityPool.Add(new UnityEnhancedVfxVisual
                {
                    Object = streak,
                    Filter = filter,
                    Renderer = renderer
                });
            }

            return speedReadabilityPool[index];
        }

        private void ConfigureSpeedReadabilityVisual(
            UnityEnhancedVfxVisual visual,
            Camera camera,
            ulong tick,
            int index,
            float speedBlend)
        {
            float fieldOfView = camera.fieldOfView;
            if (fieldOfView <= 1f || fieldOfView >= 179f ||
                float.IsNaN(fieldOfView) || float.IsInfinity(fieldOfView))
            {
                fieldOfView = 70f;
            }

            float halfHeight = Mathf.Tan(fieldOfView * Mathf.Deg2Rad * 0.5f) *
                               SpeedReadabilityDepth;
            float halfWidth = halfHeight * Mathf.Max(0.1f, camera.aspect);
            float tickPhase = (tick % 120UL) / 120f;
            float phase = index * 2.399963f + tickPhase * 0.32f;
            float radius = Mathf.Lerp(
                SpeedReadabilityMinimumRadius,
                SpeedReadabilityMaximumRadius,
                Mathf.Repeat(index * 0.37f + tickPhase * 0.18f, 1f));
            float length = Mathf.Lerp(
                SpeedReadabilityMinimumLength,
                SpeedReadabilityMaximumLength,
                Mathf.Clamp01(speedBlend * 0.72f +
                              Mathf.Repeat(index * 0.29f + tickPhase, 1f) * 0.28f));
            float width = Mathf.Lerp(
                SpeedReadabilityMinimumWidth,
                SpeedReadabilityMaximumWidth,
                Mathf.Repeat(index * 0.43f + tickPhase * 0.25f, 1f));

            visual.Object.transform.localPosition = new Vector3(
                Mathf.Cos(phase) * radius * halfWidth,
                Mathf.Sin(phase) * radius * halfHeight,
                SpeedReadabilityDepth);
            visual.Object.transform.localRotation = Quaternion.Euler(
                0f,
                0f,
                phase * Mathf.Rad2Deg);
            visual.Object.transform.localScale = new Vector3(
                halfWidth * 2f * width,
                halfHeight * 2f * length,
                1f);
            visual.Filter.sharedMesh = GetMesh(UnityEnhancedVfxKind.SpeedTrail);
            visual.Renderer.sharedMaterial = GetMaterial(UnityEnhancedVfxKind.SpeedTrail);
        }

        private Mesh GetMesh(UnityEnhancedVfxKind kind)
        {
            Mesh mesh;
            if (meshes.TryGetValue(kind, out mesh))
            {
                return mesh;
            }

            if (kind == UnityEnhancedVfxKind.Fire)
            {
                mesh = CreateFireballMesh();
                meshes.Add(kind, mesh);
                return mesh;
            }

            if (kind == UnityEnhancedVfxKind.Impact)
            {
                mesh = CreateImpactMesh();
                meshes.Add(kind, mesh);
                return mesh;
            }

            if (kind == UnityEnhancedVfxKind.SpeedTrail)
            {
                mesh = CreateSpeedStreakMesh();
                meshes.Add(kind, mesh);
                return mesh;
            }

            mesh = new Mesh { name = "EnhancedVfxMesh_" + kind, hideFlags = HideFlags.DontSave };
            float radius;
            switch (kind)
            {
                case UnityEnhancedVfxKind.Destruction:
                    radius = 0.60f;
                    break;
                case UnityEnhancedVfxKind.SpellCast:
                    radius = 0.32f;
                    break;
                case UnityEnhancedVfxKind.Smoke:
                    radius = 0.9f;
                    break;
                default:
                    radius = 0.45f;
                    break;
            }
            mesh.vertices = new[]
            {
                new Vector3(-radius, 0f, -radius), new Vector3(radius, 0f, -radius),
                new Vector3(radius, 0f, radius), new Vector3(-radius, 0f, radius),
                new Vector3(0f, radius * 2f, 0f)
            };
            mesh.triangles = new[]
            {
                0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4
            };
            mesh.RecalculateNormals();
            meshes.Add(kind, mesh);
            return mesh;
        }

        private static Mesh CreateSpeedStreakMesh()
        {
            Mesh mesh = new Mesh
            {
                name = "EnhancedVfxMesh_SpeedTrail",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                new Vector3(0f, -0.5f, 0f),
                new Vector3(0.5f, 0f, 0f),
                new Vector3(0f, 0.5f, 0f),
                new Vector3(-0.5f, 0f, 0f)
            };
            mesh.triangles = new[] { 0, 1, 2, 0, 2, 3 };
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private static Mesh CreateFireballMesh()
        {
            const float half = 0.72f;
            Mesh mesh = new Mesh
            {
                name = "EnhancedVfxMesh_Fireball",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                new Vector3(-half, -half, 0f), new Vector3(half, -half, 0f),
                new Vector3(half, half, 0f), new Vector3(-half, half, 0f),
                new Vector3(0f, -half, -half), new Vector3(0f, half, -half),
                new Vector3(0f, -half, half), new Vector3(0f, half, half)
            };
            mesh.uv = new[]
            {
                new Vector2(0f, 0f), new Vector2(1f, 0f),
                new Vector2(1f, 1f), new Vector2(0f, 1f),
                new Vector2(0f, 0f), new Vector2(0f, 1f),
                new Vector2(1f, 0f), new Vector2(1f, 1f)
            };
            mesh.triangles = new[]
            {
                0, 2, 1, 0, 3, 2,
                4, 5, 7, 4, 7, 6
            };
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private static Mesh CreateImpactMesh()
        {
            const int ringSegments = 12;
            const float ringInnerRadius = 0.16f;
            const float ringOuterRadius = 0.42f;
            const float ringHeight = 0.045f;
            const float burstBaseRadius = 0.08f;
            const float burstHalfWidth = 0.12f;
            const float burstTipRadius = 0.43f;

            const float billboardCenterHeight = 0.25f;
            const float billboardInnerRadius = 0.14f;
            const float billboardOuterRadius = 0.42f;
            var vertices = new List<Vector3>(ringSegments * 6 + 24);
            var triangles = new List<int>(ringSegments * 18 + 48);
            AddImpactRing(
                vertices,
                triangles,
                Vector3.up * ringHeight,
                Vector3.right,
                Vector3.forward,
                ringInnerRadius,
                ringOuterRadius,
                ringSegments);
            AddImpactRing(
                vertices,
                triangles,
                Vector3.up * billboardCenterHeight,
                Vector3.right,
                Vector3.up,
                billboardInnerRadius,
                billboardOuterRadius,
                ringSegments);
            AddImpactRing(
                vertices,
                triangles,
                Vector3.up * billboardCenterHeight,
                Vector3.forward,
                Vector3.up,
                billboardInnerRadius,
                billboardOuterRadius,
                ringSegments);
            AddImpactDisc(
                vertices,
                triangles,
                Vector3.up * billboardCenterHeight,
                Vector3.right,
                Vector3.up,
                0.24f,
                ringSegments);

            for (int index = 0; index < 8; index++)
            {
                float angle = index * Mathf.PI * 0.25f + Mathf.PI * 0.125f;
                Vector3 direction = new Vector3(Mathf.Cos(angle), Mathf.Sin(angle), 0f);
                Vector3 side = new Vector3(-direction.y, direction.x, 0f) * burstHalfWidth;
                int baseIndex = vertices.Count;
                Vector3 burstCenter = Vector3.up * billboardCenterHeight;
                Vector3 burstBase = burstCenter + direction * burstBaseRadius;
                Vector3 burstTip = burstCenter + direction * burstTipRadius;
                vertices.Add(burstBase - side);
                vertices.Add(burstBase + side);
                vertices.Add(burstTip);
                triangles.Add(baseIndex);
                triangles.Add(baseIndex + 1);
                triangles.Add(baseIndex + 2);
            }

            Mesh mesh = new Mesh
            {
                name = "EnhancedVfxMesh_ImpactBurst",
                hideFlags = HideFlags.DontSave
            };
            mesh.SetVertices(vertices);
            mesh.SetTriangles(triangles, 0);
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            return mesh;
        }

        private static void AddImpactDisc(
            List<Vector3> vertices,
            List<int> triangles,
            Vector3 center,
            Vector3 axisA,
            Vector3 axisB,
            float radius,
            int segments)
        {
            int centerIndex = vertices.Count;
            vertices.Add(center);
            int perimeterStart = vertices.Count;
            for (int index = 0; index < segments; index++)
            {
                float angle = index * Mathf.PI * 2f / segments;
                Vector3 direction = axisA * Mathf.Cos(angle) + axisB * Mathf.Sin(angle);
                vertices.Add(center + direction * radius);
            }

            for (int index = 0; index < segments; index++)
            {
                int next = (index + 1) % segments;
                triangles.Add(centerIndex);
                triangles.Add(perimeterStart + index);
                triangles.Add(perimeterStart + next);
            }
        }

        private static void AddImpactRing(
            List<Vector3> vertices,
            List<int> triangles,
            Vector3 center,
            Vector3 axisA,
            Vector3 axisB,
            float innerRadius,
            float outerRadius,
            int segments)
        {
            int start = vertices.Count;
            for (int index = 0; index < segments; index++)
            {
                float angle = index * Mathf.PI * 2f / segments;
                Vector3 direction = axisA * Mathf.Cos(angle) + axisB * Mathf.Sin(angle);
                vertices.Add(center + direction * innerRadius);
                vertices.Add(center + direction * outerRadius);
            }

            for (int index = 0; index < segments; index++)
            {
                int next = (index + 1) % segments;
                int inner = start + index * 2;
                int outer = inner + 1;
                int nextInner = start + next * 2;
                int nextOuter = nextInner + 1;
                triangles.Add(inner);
                triangles.Add(outer);
                triangles.Add(nextOuter);
                triangles.Add(inner);
                triangles.Add(nextOuter);
                triangles.Add(nextInner);
            }
        }

        private Material GetMaterial(UnityEnhancedVfxKind kind)
        {
            Material material;
            if (materials.TryGetValue(kind, out material))
            {
                return material;
            }

            Shader customFireballShader = kind == UnityEnhancedVfxKind.Fire
                ? Shader.Find("MagicCarpet/Enhanced/Fireball")
                : null;
            Shader customImpactShader = kind == UnityEnhancedVfxKind.Impact
                ? Shader.Find("MagicCarpet/Enhanced/Impact")
                : null;
            Shader customVfxShader = Shader.Find("MagicCarpet/Enhanced/Vfx");
            Shader shader = customFireballShader ?? customImpactShader ?? customVfxShader ??
                            Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
                            Shader.Find("Universal Render Pipeline/Lit") ??
                            Shader.Find("Standard");
            material = shader == null ? null : new Material(shader)
            {
                name = "EnhancedVfxMaterial_" + kind,
                hideFlags = HideFlags.DontSave,
                color = VfxColor(kind)
            };
            if (material != null)
            {
                material.enableInstancing = true;
                if (shader == customFireballShader)
                {
                    material.SetColor("_CoreColor", new Color(1f, 0.72f, 0.22f, 1f));
                    material.SetColor("_HaloColor", new Color(1f, 0.06f, 0.008f, 1f));
                    material.SetFloat("_CoreIntensity", 0f);
                    material.SetFloat("_HaloIntensity", 4.6f);
                    material.SetFloat("_CoreRadius", 0.08f);
                    material.SetFloat("_HaloRadius", 0.98f);
                    material.SetFloat("_OuterRadius", 1.56f);
                    material.SetFloat("_OuterIntensity", 6.2f);
                    material.SetFloat("_Opacity", 0.90f);
                    material.SetFloat("_PulseSpeed", 3.0f);
                    material.SetFloat("_ShimmerStrength", 0.24f);
                }
                else if (shader == customImpactShader)
                {
                    material.SetColor("_GlowColor", VfxColor(kind));
                    material.SetFloat("_GlowIntensity", 2.8f);
                    material.SetFloat("_Opacity", 0.92f);
                    material.SetFloat("_PulseSpeed", 5.0f);
                    material.SetFloat("_RimStrength", 1.8f);
                }
                else if (shader == customVfxShader)
                {
                    float glow = kind == UnityEnhancedVfxKind.SpeedTrail ? 1.1f :
                                 kind == UnityEnhancedVfxKind.Weather ? 0.7f :
                                 kind == UnityEnhancedVfxKind.Smoke ? 0.28f :
                                 kind == UnityEnhancedVfxKind.Fire ? 1.5f :
                                 kind == UnityEnhancedVfxKind.SpellCast ? 1.15f :
                                 kind == UnityEnhancedVfxKind.Impact ? 2.0f : 1.6f;
                    float opacity = kind == UnityEnhancedVfxKind.SpeedTrail ? 0.26f :
                                    kind == UnityEnhancedVfxKind.Smoke ? 0.28f :
                                    kind == UnityEnhancedVfxKind.Impact ? 0.80f :
                                    kind == UnityEnhancedVfxKind.SpellCast ? 0.65f : 0.72f;
                    material.SetColor("_GlowColor", VfxColor(kind));
                    material.SetFloat("_GlowIntensity", glow);
                    material.SetFloat("_Opacity", opacity);
                    material.SetFloat(
                        "_PulseSpeed",
                        kind == UnityEnhancedVfxKind.SpeedTrail ? 0f :
                        kind == UnityEnhancedVfxKind.Impact ? 5.0f : 2.5f);
                    material.SetFloat(
                        "_RimStrength",
                        kind == UnityEnhancedVfxKind.SpeedTrail ? 0.90f :
                        kind == UnityEnhancedVfxKind.Impact ? 1.8f : 1.2f);
                }
                else
                {
                    material.SetFloat("_AmbientBoost", 0.58f);
                    material.SetFloat("_SpecularStrength", 0.06f);
                    material.SetColor("_RimColor", new Color(0.08f, 0.16f, 0.52f, 1f));
                    material.SetFloat("_RimStrength", 0.08f);
                    material.SetColor("_EmissionColor", VfxColor(kind));
                    material.SetFloat("_EmissionStrength", kind == UnityEnhancedVfxKind.Weather ? 0.05f : 0.38f);
                }
            }
            materials.Add(kind, material);
            return material;
        }

        private static Color VfxColor(UnityEnhancedVfxKind kind)
        {
            switch (kind)
            {
                case UnityEnhancedVfxKind.SpellCast: return new Color(0.65f, 0.25f, 1f);
                case UnityEnhancedVfxKind.ManaPickup: return new Color(0.25f, 0.9f, 1f);
                case UnityEnhancedVfxKind.Destruction: return new Color(1f, 0.35f, 0.08f);
                case UnityEnhancedVfxKind.Weather: return new Color(0.35f, 0.5f, 1f);
                case UnityEnhancedVfxKind.SpeedTrail: return new Color(0.25f, 0.85f, 1f);
                case UnityEnhancedVfxKind.Impact: return new Color(1f, 0.84f, 0.18f);
                case UnityEnhancedVfxKind.Fire: return new Color(1f, 0.2f, 0.03f);
                default: return new Color(0.32f, 0.35f, 0.4f);
            }
        }

        private void RecycleAll()
        {
            foreach (UnityEnhancedVfxVisual visual in active.Values)
            {
                if (visual.Object != null)
                {
                    visual.Object.SetActive(false);
                    pool.Push(visual);
                }
            }
            active.Clear();
        }

        private void OnDestroy()
        {
            foreach (Mesh mesh in meshes.Values)
            {
                if (mesh != null) DestroyImmediate(mesh);
            }
            foreach (Material material in materials.Values)
            {
                if (material != null) DestroyImmediate(material);
            }
        }
    }
}
