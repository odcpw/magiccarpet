using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace MagicCarpet.PresentationLab
{
    /// <summary>
    /// One qualified owner billboard selected for an Enhanced actor snapshot.
    /// The mesh is presentation-owned; the indexed pixels and palette remain
    /// external owner-cache inputs.
    /// </summary>
    internal sealed class UnityEnhancedOwnerActorSprite
    {
        public Material Material;
        public Mesh Mesh;
        public float Width;
        public float Height;
    }

    /// <summary>
    /// Reads only the already-qualified Level 1 owner actor catalog. This is an
    /// Enhanced presentation adapter, not a second gameplay/entity authority.
    /// Native session opening has already validated the complete cache identity;
    /// this reader additionally checks the cache scope, schema, and confined
    /// files before creating Unity textures.
    /// </summary>
    internal sealed class UnityEnhancedOwnerActorCatalog : IDisposable
    {
        private const string CacheSchema = "mc2-owner-unity-asset-cache/v1";
        private const string PrivateOwnerScope = "PRIVATE_TRACK_P_OWNER_DATA";
        private const string PresentationSchema = "mc2-level1-entity-presentations/v1";
        private const float SourceUnitsPerUnityUnit = 64f;
        private const float EnhancedVerticalScale = 0.5f;

        private readonly Dictionary<ushort, OwnerActorDescriptor> descriptors =
            new Dictionary<ushort, OwnerActorDescriptor>();
        private readonly Dictionary<int, OwnerTextureDescriptor> textures =
            new Dictionary<int, OwnerTextureDescriptor>();
        private readonly Dictionary<int, Texture2D> indexTextures =
            new Dictionary<int, Texture2D>();
        private readonly Dictionary<int, Material> materials =
            new Dictionary<int, Material>();

        private Texture2D paletteTexture;
        private Mesh normalQuad;
        private Mesh mirroredQuad;
        private string cacheRoot;

        public bool IsConfigured { get; private set; }
        public int QualifiedActorCount { get { return descriptors.Count; } }

        public bool Configure(string cacheRoot, out string reason)
        {
            Dispose();
            reason = string.Empty;

            if (string.IsNullOrWhiteSpace(cacheRoot) || !Directory.Exists(cacheRoot))
            {
                reason = "owner actor cache directory does not exist";
                return false;
            }

            try
            {
                cacheRoot = Path.GetFullPath(cacheRoot);
                OwnerCacheManifest manifest = ReadJson<OwnerCacheManifest>(
                    ConfinedPath(cacheRoot, "manifest.json"));
                if (manifest == null || manifest.schema != CacheSchema ||
                    manifest.scope != PrivateOwnerScope)
                {
                    reason = "owner actor cache schema or private scope is not qualified";
                    return false;
                }

                EntityPresentationDescriptor presentation = ReadJson<EntityPresentationDescriptor>(
                    ConfinedPath(cacheRoot, "entities/presentation.json"));
                if (presentation == null || presentation.schema != PresentationSchema ||
                    presentation.groups == null)
                {
                    reason = "owner actor presentation catalog is not qualified";
                    return false;
                }

                TerrainMaterialsDescriptor terrainMaterials =
                    ReadJson<TerrainMaterialsDescriptor>(
                        ConfinedPath(cacheRoot, "terrain/materials.json"));
                if (terrainMaterials == null || terrainMaterials.tmaps1 == null ||
                    terrainMaterials.tmaps1.textures == null)
                {
                    reason = "owner actor TMAP catalog is missing";
                    return false;
                }

                for (int index = 0; index < terrainMaterials.tmaps1.textures.Length; index++)
                {
                    OwnerTextureDescriptor row = terrainMaterials.tmaps1.textures[index];
                    if (row == null || row.status != "decoded_indexed_texture" ||
                        row.index < 0 || row.width <= 0 || row.height <= 0 ||
                        string.IsNullOrWhiteSpace(row.indices_path) ||
                        textures.ContainsKey(row.index))
                    {
                        continue;
                    }

                    textures.Add(row.index, row);
                }

                byte[] paletteBytes = File.ReadAllBytes(
                    ConfinedPath(cacheRoot, "palette/paln.rgb8"));
                if (paletteBytes.Length != 256 * 3)
                {
                    reason = "owner actor PALN palette must contain 768 RGB8 bytes";
                    return false;
                }

                paletteTexture = new Texture2D(
                    256,
                    1,
                    TextureFormat.RGB24,
                    false,
                    true)
                {
                    name = "Owner Indexed Actor PALN",
                    filterMode = FilterMode.Point,
                    wrapMode = TextureWrapMode.Clamp
                };
                paletteTexture.LoadRawTextureData(paletteBytes);
                paletteTexture.Apply(false, true);

                for (int index = 0; index < presentation.groups.Length; index++)
                {
                    EntityPresentationGroup group = presentation.groups[index];
                    if (!IsSupportedGroup(group))
                    {
                        continue;
                    }

                    if (group.qualified_view_entity_indices == null ||
                        group.qualified_view_entity_indices.Length == 0)
                    {
                        continue;
                    }

                    OwnerActorDescriptor descriptor = new OwnerActorDescriptor
                    {
                        BaseSelector = group.particle.base_tmap_selector,
                        LogicalHeightSourceUnits = group.particle.logical_height_source_units,
                        RenderMode = group.particle.render_mode,
                        QualifiedSelectors = group.selector.qualified_selectors,
                        YawLookup = group.selector.yaw_lookup_0_15,
                        MirrorWhenRear = group.selector.mirror_when_orientation_gte_8,
                        Semantic = group.semantic_family
                    };

                    if (!descriptor.IsValid)
                    {
                        reason = "owner actor group has an incomplete billboard selector contract";
                        Dispose();
                        return false;
                    }

                    for (int entityOrdinal = 0;
                         entityOrdinal < group.qualified_view_entity_indices.Length;
                         entityOrdinal++)
                    {
                        int rawEntityIndex = group.qualified_view_entity_indices[entityOrdinal];
                        if (rawEntityIndex < 0 || rawEntityIndex > ushort.MaxValue)
                        {
                            reason = "owner actor entity index is outside the snapshot range";
                            Dispose();
                            return false;
                        }

                        ushort entityIndex = (ushort)rawEntityIndex;
                        if (descriptors.ContainsKey(entityIndex))
                        {
                            reason = "owner actor catalog contains duplicate qualified entity indices";
                            Dispose();
                            return false;
                        }

                        descriptors.Add(entityIndex, descriptor);
                    }
                }

                if (descriptors.Count != presentation.qualified_actor_count ||
                    descriptors.Count != 64)
                {
                    reason = "owner actor catalog does not contain the qualified 64-actor Level 1 view";
                    Dispose();
                    return false;
                }

                foreach (OwnerActorDescriptor descriptor in descriptors.Values)
                {
                    for (int selectorIndex = 0;
                         selectorIndex < descriptor.QualifiedSelectors.Length;
                         selectorIndex++)
                    {
                        int selector = descriptor.QualifiedSelectors[selectorIndex];
                        if (!textures.ContainsKey(selector))
                        {
                            reason = "owner actor selector is missing a decoded indexed TMAP row";
                            Dispose();
                            return false;
                        }
                    }
                }

                this.cacheRoot = cacheRoot;
                IsConfigured = true;
                return true;
            }
            catch (Exception exception)
            {
                reason = "owner actor cache load failed: " + exception.Message;
                Dispose();
                return false;
            }
        }

        public bool TryResolve(
            UnityEnhancedEntitySnapshot entity,
            out UnityEnhancedOwnerActorSprite sprite)
        {
            sprite = null;
            if (!IsConfigured || entity == null ||
                !descriptors.TryGetValue(entity.SourceEntityIndex, out OwnerActorDescriptor descriptor))
            {
                return false;
            }

            int selector = descriptor.BaseSelector;
            bool mirrored = false;
            if (descriptor.RenderMode == 20)
            {
                int orientation = (((entity.SourceYaw - entity.CameraYaw) >> 3) & 0xf0) >> 4;
                if (orientation < 0 || orientation >= descriptor.YawLookup.Length)
                {
                    return false;
                }

                selector += descriptor.YawLookup[orientation];
                mirrored = orientation >= 8 && descriptor.MirrorWhenRear;
            }

            if (!Contains(descriptor.QualifiedSelectors, selector) ||
                !textures.TryGetValue(selector, out OwnerTextureDescriptor row))
            {
                return false;
            }

            Texture2D indexTexture = GetIndexTexture(row);
            Material material = GetMaterial(selector, indexTexture);
            float height = descriptor.LogicalHeightSourceUnits / SourceUnitsPerUnityUnit *
                           EnhancedVerticalScale;
            float width = height * row.width / row.height;
            sprite = new UnityEnhancedOwnerActorSprite
            {
                Material = material,
                Mesh = GetQuad(mirrored),
                Width = width,
                Height = height
            };
            return true;
        }

        public void Dispose()
        {
            IsConfigured = false;
            descriptors.Clear();
            textures.Clear();

            foreach (Material material in materials.Values)
            {
                DestroyObject(material);
            }
            materials.Clear();

            foreach (Texture2D texture in indexTextures.Values)
            {
                DestroyObject(texture);
            }
            indexTextures.Clear();

            DestroyObject(paletteTexture);
            paletteTexture = null;
            DestroyObject(normalQuad);
            normalQuad = null;
            DestroyObject(mirroredQuad);
            mirroredQuad = null;
            cacheRoot = null;
        }

        private Texture2D GetIndexTexture(OwnerTextureDescriptor row)
        {
            Texture2D texture;
            if (indexTextures.TryGetValue(row.index, out texture))
            {
                return texture;
            }

            byte[] bytes = File.ReadAllBytes(ConfinedPath(cacheRoot, row.indices_path));
            if (bytes.Length != row.width * row.height)
            {
                throw new InvalidDataException(
                    "owner actor TMAP extent does not match its catalog row");
            }

            texture = new Texture2D(row.width, row.height, TextureFormat.R8, false, true)
            {
                name = "Owner Indexed Actor TMAP " + row.index.ToString("D3"),
                filterMode = FilterMode.Point,
                wrapMode = TextureWrapMode.Clamp
            };
            texture.LoadRawTextureData(bytes);
            texture.Apply(false, true);
            indexTextures.Add(row.index, texture);
            return texture;
        }

        private Material GetMaterial(int selector, Texture2D indexTexture)
        {
            Material material;
            if (materials.TryGetValue(selector, out material))
            {
                return material;
            }

            Shader shader = Shader.Find("MagicCarpet/Enhanced/OwnerEntitySprite");
            if (shader == null)
            {
                throw new InvalidOperationException("owner actor indexed billboard shader is missing");
            }

            material = new Material(shader)
            {
                name = "Owner Indexed Actor Material " + selector.ToString("D3"),
                hideFlags = HideFlags.DontSave
            };
            material.SetTexture("_Indices", indexTexture);
            material.SetTexture("_Palette", paletteTexture);
            material.SetColor("_Tint", new Color(0.92f, 0.96f, 1f, 1f));
            material.SetFloat("_EmissionStrength", 0.10f);
            materials.Add(selector, material);
            return material;
        }

        private Mesh GetQuad(bool mirrored)
        {
            if (mirrored)
            {
                if (mirroredQuad == null) mirroredQuad = CreateQuad(true);
                return mirroredQuad;
            }

            if (normalQuad == null) normalQuad = CreateQuad(false);
            return normalQuad;
        }

        private static Mesh CreateQuad(bool mirrored)
        {
            Mesh mesh = new Mesh
            {
                name = mirrored
                    ? "Owner Indexed Actor Billboard Quad Mirrored"
                    : "Owner Indexed Actor Billboard Quad",
                hideFlags = HideFlags.DontSave
            };
            mesh.vertices = new[]
            {
                new Vector3(-0.5f, -0.5f, 0f),
                new Vector3(0.5f, -0.5f, 0f),
                new Vector3(-0.5f, 0.5f, 0f),
                new Vector3(0.5f, 0.5f, 0f)
            };
            mesh.uv = mirrored
                ? new[]
                {
                    new Vector2(1f, 1f), new Vector2(0f, 1f),
                    new Vector2(1f, 0f), new Vector2(0f, 0f)
                }
                : new[]
                {
                    new Vector2(0f, 1f), new Vector2(1f, 1f),
                    new Vector2(0f, 0f), new Vector2(1f, 0f)
                };
            mesh.triangles = new[] { 0, 2, 1, 2, 3, 1 };
            mesh.RecalculateBounds();
            return mesh;
        }

        private static bool IsSupportedGroup(EntityPresentationGroup group)
        {
            if (group == null || group.authority != "FrozenObserved" ||
                group.particle == null || group.selector == null ||
                group.billboard == null || group.qualified_view_count <= 0 ||
                group.billboard.facing != "full_camera_facing_screen_plane")
            {
                return false;
            }

            return group.semantic_family == "goat" ||
                   group.semantic_family == "dolmen" ||
                   group.semantic_family == "villager-a" ||
                   group.semantic_family == "villager-b" ||
                   group.semantic_family == "villager-c" ||
                   group.semantic_family == "villager-d";
        }

        private static bool Contains(int[] values, int value)
        {
            if (values == null) return false;
            for (int index = 0; index < values.Length; index++)
            {
                if (values[index] == value) return true;
            }

            return false;
        }

        private static T ReadJson<T>(string path) where T : class
        {
            return JsonUtility.FromJson<T>(File.ReadAllText(path));
        }

        private static string ConfinedPath(string root, string relative)
        {
            if (string.IsNullOrWhiteSpace(relative) || Path.IsPathRooted(relative))
            {
                throw new InvalidDataException("owner actor cache path is not relative");
            }

            string fullRoot = Path.GetFullPath(root).TrimEnd(
                Path.DirectorySeparatorChar,
                Path.AltDirectorySeparatorChar);
            string candidate = Path.GetFullPath(Path.Combine(fullRoot, relative));
            string prefix = fullRoot + Path.DirectorySeparatorChar;
            if (!candidate.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException("owner actor cache path escapes its root");
            }

            return candidate;
        }

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }

        [Serializable]
        private sealed class OwnerCacheManifest
        {
            public string schema;
            public string scope;
        }

        [Serializable]
        private sealed class EntityPresentationDescriptor
        {
            public string schema;
            public int qualified_actor_count;
            public EntityPresentationGroup[] groups;
        }

        [Serializable]
        private sealed class EntityPresentationGroup
        {
            public string authority;
            public string semantic_family;
            public int qualified_view_count;
            public int[] qualified_view_entity_indices;
            public EntityParticleDescriptor particle;
            public EntitySelectorDescriptor selector;
            public EntityBillboardDescriptor billboard;
        }

        [Serializable]
        private sealed class EntityParticleDescriptor
        {
            public int base_tmap_selector;
            public int logical_height_source_units;
            public int render_mode;
        }

        [Serializable]
        private sealed class EntitySelectorDescriptor
        {
            public int[] qualified_selectors;
            public int[] yaw_lookup_0_15;
            public bool mirror_when_orientation_gte_8;
        }

        [Serializable]
        private sealed class EntityBillboardDescriptor
        {
            public string facing;
        }

        [Serializable]
        private sealed class TerrainMaterialsDescriptor
        {
            public string schema;
            public TmapCollection tmaps1;
        }

        [Serializable]
        private sealed class TmapCollection
        {
            public OwnerTextureDescriptor[] textures;
        }

        [Serializable]
        private sealed class OwnerTextureDescriptor
        {
            public int index;
            public int width;
            public int height;
            public string indices_path;
            public string status;
        }

        private sealed class OwnerActorDescriptor
        {
            public int BaseSelector;
            public int LogicalHeightSourceUnits;
            public int RenderMode;
            public int[] QualifiedSelectors;
            public int[] YawLookup;
            public bool MirrorWhenRear;
            public string Semantic;

            public bool IsValid
            {
                get
                {
                    return BaseSelector >= 0 && LogicalHeightSourceUnits > 0 &&
                           QualifiedSelectors != null && QualifiedSelectors.Length > 0 &&
                           (RenderMode != 20 || (YawLookup != null && YawLookup.Length == 16)) &&
                           !string.IsNullOrWhiteSpace(Semantic);
                }
            }
        }
    }
}
