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
        public int Family = -1;

        public void Release()
        {
            if (VisualObject != null)
            {
                VisualObject.SetActive(false);
            }
        }
    }

    /// <summary>
    /// Supplies pooled, generated billboards/meshes for all Level 1 Night
    /// families.  It intentionally has no imported textures or authored
    /// character payloads; those can replace the generated definitions later.
    /// </summary>
    [DisallowMultipleComponent]
    public sealed class UnityEnhancedActorPresentation : MonoBehaviour
    {
        private readonly Dictionary<int, Mesh> meshes = new Dictionary<int, Mesh>();
        private readonly Dictionary<int, Material> materials = new Dictionary<int, Material>();
        private readonly HashSet<GameObject> configuredViews = new HashSet<GameObject>();

        public int ActiveVisualCount { get; private set; }
        public int PooledVisualCount { get; private set; }
        public int LastConfiguredFamily { get; private set; } = -1;
        public int LastSortingOrder { get; private set; }

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

            if (state.Family != entity.PresentationFamily)
            {
                state.Family = entity.PresentationFamily;
                state.Filter.sharedMesh = GetMesh(entity.PresentationFamily);
                state.Renderer.sharedMaterial = GetMaterial(entity.PresentationFamily);
            }

            state.VisualObject.SetActive(entity.Visible);
            state.VisualObject.transform.localRotation = Quaternion.Euler(0f, entity.FacingDegrees, 0f);
            float scale = entity.Scale <= 0f ? 1f : entity.Scale;
            float animationOffset = IsAnimated(entity.PresentationFamily)
                ? Mathf.Sin(entity.AnimationPhase * Mathf.PI * 2f) * 0.05f
                : 0f;
            state.VisualObject.transform.localPosition = new Vector3(0f, animationOffset, 0f);
            state.VisualObject.transform.localScale = Vector3.one * scale;
            state.Renderer.sortingOrder = entity.RenderLayer;
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
        }

        private Mesh GetMesh(int family)
        {
            Mesh mesh;
            if (meshes.TryGetValue(family, out mesh))
            {
                return mesh;
            }

            UnityEnhancedPresentationFamily familyId = (UnityEnhancedPresentationFamily)family;
            mesh = IsBillboard(familyId) ? CreateBillboardMesh(familyId) : CreateBoxMesh(familyId);
            mesh.name = "EnhancedGeneratedMesh_" + familyId;
            mesh.hideFlags = HideFlags.DontSave;
            meshes.Add(family, mesh);
            return mesh;
        }

        private Material GetMaterial(int family)
        {
            Material material;
            if (materials.TryGetValue(family, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/VertexColorLit") ??
                            Shader.Find("Universal Render Pipeline/Lit") ??
                            Shader.Find("Standard");
            material = shader == null ? null : new Material(shader)
            {
                name = "EnhancedGeneratedMaterial_" + (UnityEnhancedPresentationFamily)family,
                hideFlags = HideFlags.DontSave,
                color = FamilyColor((UnityEnhancedPresentationFamily)family)
            };
            if (material != null)
            {
                material.enableInstancing = true;
                material.SetFloat("_AmbientBoost", 0.62f);
                material.SetFloat("_SpecularStrength", 0.12f);
                material.SetColor("_RimColor", new Color(0.08f, 0.16f, 0.52f, 1f));
                material.SetFloat("_RimStrength", 0.24f);
                material.SetColor("_EmissionColor", FamilyColor((UnityEnhancedPresentationFamily)family));
                material.SetFloat(
                    "_EmissionStrength",
                    EmissionStrength((UnityEnhancedPresentationFamily)family));
            }
            materials.Add(family, material);
            return material;
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
            float half = family == UnityEnhancedPresentationFamily.Smoke ? 0.7f : 0.35f;
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

            Vector3[] vertices =
            {
                new Vector3(-halfExtents.x, 0f, -halfExtents.z), new Vector3(halfExtents.x, 0f, -halfExtents.z),
                new Vector3(halfExtents.x, halfExtents.y * 2f, -halfExtents.z), new Vector3(-halfExtents.x, halfExtents.y * 2f, -halfExtents.z),
                new Vector3(-halfExtents.x, 0f, halfExtents.z), new Vector3(halfExtents.x, 0f, halfExtents.z),
                new Vector3(halfExtents.x, halfExtents.y * 2f, halfExtents.z), new Vector3(-halfExtents.x, halfExtents.y * 2f, halfExtents.z)
            };
            Mesh mesh = new Mesh();
            mesh.vertices = vertices;
            mesh.triangles = new[]
            {
                0, 2, 1, 0, 3, 2,
                1, 2, 6, 1, 6, 5,
                5, 6, 7, 5, 7, 4,
                4, 7, 3, 4, 3, 0,
                3, 7, 6, 3, 6, 2,
                4, 0, 1, 4, 1, 5
            };
            mesh.RecalculateNormals();
            return mesh;
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
