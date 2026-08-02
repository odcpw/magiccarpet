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
        private readonly Stack<UnityEnhancedVfxVisual> pool = new Stack<UnityEnhancedVfxVisual>();
        private readonly Dictionary<ulong, UnityEnhancedVfxVisual> active = new Dictionary<ulong, UnityEnhancedVfxVisual>();
        private readonly Dictionary<UnityEnhancedVfxKind, Mesh> meshes = new Dictionary<UnityEnhancedVfxKind, Mesh>();
        private readonly Dictionary<UnityEnhancedVfxKind, Material> materials = new Dictionary<UnityEnhancedVfxKind, Material>();
        private Transform effectRoot;
        private ulong generation;
        private ulong lastEventSequence;
        private ulong lastTick;
        private bool reducedMotion;

        public int MaxActiveEffects { get; set; } = 128;
        public int ActiveEffectCount { get { return active.Count; } }
        public int PooledEffectCount { get { return pool.Count; } }
        public int DroppedEffectCount { get; private set; }
        public int DuplicateEffectCount { get; private set; }
        public ulong Generation { get { return generation; } }
        public ulong LastEventSequence { get { return lastEventSequence; } }
        public bool ReducedMotion { get { return reducedMotion; } }
        public bool CameraComfortEnabled { get; private set; } = true;

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
            float intensity = Mathf.Clamp(vfxEvent.Intensity <= 0f ? 1f : vfxEvent.Intensity, 0.1f, 4f);
            visual.Object.transform.localScale = Vector3.one * intensity * (reducedMotion ? 0.6f : 1f);
            visual.Filter.sharedMesh = GetMesh(vfxEvent.Kind);
            visual.Renderer.sharedMaterial = GetMaterial(vfxEvent.Kind);
            visual.Object.SetActive(true);
            active.Add(vfxEvent.Sequence, visual);
            return true;
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

        private Mesh GetMesh(UnityEnhancedVfxKind kind)
        {
            Mesh mesh;
            if (meshes.TryGetValue(kind, out mesh))
            {
                return mesh;
            }

            mesh = new Mesh { name = "EnhancedVfxMesh_" + kind, hideFlags = HideFlags.DontSave };
            float radius;
            switch (kind)
            {
                case UnityEnhancedVfxKind.Fire:
                case UnityEnhancedVfxKind.Destruction:
                    radius = 0.85f;
                    break;
                case UnityEnhancedVfxKind.SpellCast:
                    radius = 0.65f;
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

        private Material GetMaterial(UnityEnhancedVfxKind kind)
        {
            Material material;
            if (materials.TryGetValue(kind, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
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
                material.SetFloat("_AmbientBoost", 0.58f);
                material.SetFloat("_SpecularStrength", 0.06f);
                material.SetColor("_RimColor", new Color(0.08f, 0.16f, 0.52f, 1f));
                material.SetFloat("_RimStrength", 0.08f);
                material.SetColor("_EmissionColor", VfxColor(kind));
                material.SetFloat("_EmissionStrength", kind == UnityEnhancedVfxKind.Weather ? 0.05f : 0.38f);
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
