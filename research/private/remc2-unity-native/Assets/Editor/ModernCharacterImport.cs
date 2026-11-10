using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

namespace MagicCarpet.Native.Editor
{
    // Import the owner-local character exports through Unity's native FBX path.
    public sealed class ModernCharacterImport : AssetPostprocessor
    {
        private const string Folder = "Assets/Resources/ModernCharacters/";
        private const string ConformanceFolder = "Assets/Editor/CharacterConformance/";

        private void OnPreprocessModel()
        {
            // Diagnostic FBXs use the same import contract while remaining
            // editor-only, outside the Resources packaged into the player.
            if (!assetPath.StartsWith(Folder, StringComparison.Ordinal) &&
                !(assetPath.StartsWith(ConformanceFolder, StringComparison.Ordinal) && File.Exists(Path.ChangeExtension(assetPath, ".clips.json")))) return;
            var model = (ModelImporter)assetImporter;
            model.animationType = ModelImporterAnimationType.Generic;
            model.avatarSetup = ModelImporterAvatarSetup.CreateFromThisModel;
            model.importAnimation = true;
            model.importBlendShapes = true;
            model.animationCompression = ModelImporterAnimationCompression.Off;
            // Preserve the authored quarter-frame contact keys through FBX import.
            model.resampleCurves = false;
            model.preserveHierarchy = true;
            model.optimizeGameObjects = false;
            model.importCameras = false;
            model.importLights = false;
            model.bakeAxisConversion = true;
            model.isReadable = true;
            model.materialImportMode = ModelImporterMaterialImportMode.ImportStandard;
            var manifest = Path.ChangeExtension(assetPath, ".clips.json");
            if (File.Exists(manifest))
            {
                var data = JsonUtility.FromJson<Manifest>(File.ReadAllText(manifest));
                if (data.minBoneWeight != 0)
                {
                    if (!float.IsFinite(data.minBoneWeight) || data.minBoneWeight <= 0 || data.minBoneWeight > 1)
                        throw new InvalidOperationException("Character minimum skin weight must be finite and in (0, 1]");
                    // Apply during model preprocessing; setting this afterwards
                    // can be replaced by the following import's default state.
                    model.skinWeights = ModelImporterSkinWeights.Custom;
                    model.maxBonesPerVertex = 4;
                    model.minBoneWeight = data.minBoneWeight;
                }
                var take = model.defaultClipAnimations.FirstOrDefault()?.takeName ?? "Scene";
                model.clipAnimations = data.clips.Select(clip => new ModelImporterClipAnimation {
                    name = clip.name, takeName = take, firstFrame = clip.firstFrame,
                    lastFrame = clip.lastFrame, loopTime = clip.loop,
                    keepOriginalOrientation = true, keepOriginalPositionY = true,
                    keepOriginalPositionXZ = true, lockRootRotation = true,
                    lockRootHeightY = true, lockRootPositionXZ = true
                }).ToArray();
            }
        }

        public static void Prepare()
        {
            // A reused/imported project database can lose MonoScript binding
            // while still loading the managed class. Never serialize a
            // Character asset with a zero script pointer in that state.
            const string scriptPath = "Assets/Scripts/ModernCharacterAsset.cs";
            var script = AssetDatabase.LoadAssetAtPath<MonoScript>(scriptPath);
            if (script == null || script.GetClass() != typeof(ModernCharacterAsset))
            {
                AssetDatabase.ImportAsset(scriptPath, ImportAssetOptions.ForceUpdate | ImportAssetOptions.ForceSynchronousImport);
                throw new InvalidOperationException("Character asset script import is unresolved; repair the project import database before preparing assets");
            }
            PrepareCharacter("Goat", .14844f, script);
            PrepareCharacter("Archer", 1.65f, script);
            PrepareCharacter("Villager", 0, script);
            AssetDatabase.SaveAssets();
            Validate();
            Probe();
        }

        public static void Validate()
        {
            var script = AssetDatabase.LoadAssetAtPath<MonoScript>("Assets/Scripts/ModernCharacterAsset.cs");
            if (script == null || script.GetClass() != typeof(ModernCharacterAsset))
                throw new InvalidOperationException("Character asset script binding is unresolved");
            foreach (var name in new[] { "Goat", "Archer", "Villager" })
            {
                var asset = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
                if (asset == null || asset.model == null || MonoScript.FromScriptableObject(asset) != script)
                    throw new InvalidOperationException("Imported character cannot be loaded with its actual script binding: " + name);
                foreach (var clip in name == "Goat" ? new[] { "Idle", "Walk", "Sprint", "Hit", "Death" } :
                    name == "Villager" ? new[] { "Idle", "Walk", "Run", "Hit", "Death" } :
                    new[] { "Idle", "Walk", "StandingShot", "KneelEnter", "KneelingShot", "KneelExit", "Hit", "Death" }) asset.Clip(clip);
                if (name == "Villager") asset.ResolveHitOverlayBones(asset.model.transform);
            }
        }

        private static void PrepareCharacter(string name, float walkSpeed, MonoScript script)
        {
            var folder = Folder + name + "/";
            var path = folder + name + ".fbx";
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(folder + name + ".clips.json"));
            var mapDescriptions = manifest.materials != null && manifest.materials.Length > 0 ? manifest.materials : new[] { manifest.material };
            if (mapDescriptions.Any(m => m == null || string.IsNullOrWhiteSpace(m.name)))
                throw new InvalidOperationException("Character material descriptions are missing");
            var albedoPaths = new HashSet<string>(mapDescriptions.Select(m => folder + m.albedo), StringComparer.Ordinal);
            var normalPaths = new HashSet<string>(mapDescriptions.Where(m => !string.IsNullOrWhiteSpace(m.normal)).Select(m => folder + m.normal), StringComparer.Ordinal);
            var ormPaths = new HashSet<string>(mapDescriptions.Select(m => folder + m.orm), StringComparer.Ordinal);
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate | ImportAssetOptions.ForceSynchronousImport);
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (prefab == null) throw new InvalidOperationException("Missing character FBX " + path);
            foreach (var texture in Directory.GetFiles(folder + "textures"))
            {
                if (texture.EndsWith(".meta", StringComparison.Ordinal)) continue;
                var assetPath = texture.Replace('\\', '/');
                var importer = (TextureImporter)AssetImporter.GetAtPath(assetPath);
                importer.textureType = normalPaths.Contains(assetPath) ? TextureImporterType.NormalMap : TextureImporterType.Default;
                importer.sRGBTexture = albedoPaths.Contains(assetPath);
                importer.mipmapEnabled = true;
                importer.maxTextureSize = 2048;
                importer.anisoLevel = 4;
                importer.isReadable = ormPaths.Contains(assetPath);
                importer.textureCompression = ormPaths.Contains(assetPath) ? TextureImporterCompression.Uncompressed : TextureImporterCompression.CompressedHQ;
                importer.SaveAndReimport();
            }
            var imported = new Dictionary<string, ImportedMaps>(StringComparer.Ordinal);
            foreach (var maps in mapDescriptions)
            {
                if (maps == null || string.IsNullOrWhiteSpace(maps.name) || imported.ContainsKey(maps.name))
                    throw new InvalidOperationException("Character material identities must be nonempty and unique");
                var suffix = mapDescriptions.Length == 1 ? "" : SafeMaterialName(maps.name) + "_";
                imported.Add(maps.name, ImportMaps(folder, maps, suffix));
            }
            var matched = new HashSet<string>(StringComparer.Ordinal);
            var instance = UnityEngine.Object.Instantiate(prefab);
            try
            {
                Directory.CreateDirectory(folder + "materials");
                foreach (var renderer in instance.GetComponentsInChildren<Renderer>(true))
                {
                    renderer.sharedMaterials = renderer.sharedMaterials.Select(source => {
                        var materialPath = folder + "materials/" + SafeMaterialName(source.name) + ".mat";
                        var material = AssetDatabase.LoadAssetAtPath<Material>(materialPath);
                        if (material == null) { material = new Material(Shader.Find("Universal Render Pipeline/Lit")); AssetDatabase.CreateAsset(material, materialPath); }
                        material.SetColor("_BaseColor", source.color);
                        if (imported.TryGetValue(source.name, out var maps))
                        {
                            matched.Add(source.name);
                            material.SetTexture("_BaseMap", maps.albedo); material.SetColor("_BaseColor", Color.white);
                            material.SetTexture("_BumpMap", maps.normal);
                            if (maps.normal != null) material.EnableKeyword("_NORMALMAP"); else material.DisableKeyword("_NORMALMAP");
                            material.SetTexture("_MetallicGlossMap", maps.packed); material.EnableKeyword("_METALLICSPECGLOSSMAP");
                            material.SetTexture("_OcclusionMap", maps.packed);
                            material.EnableKeyword("_OCCLUSIONMAP");
                            material.SetFloat("_Smoothness", 1); material.SetFloat("_Metallic", 1);
                        }
                        else material.SetFloat("_Smoothness", .28f);
                        EditorUtility.SetDirty(material);
                        return material;
                    }).ToArray();
                    renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.On;
                }
                if (matched.Count != imported.Count)
                    throw new InvalidOperationException("Declared character material is absent from the FBX: " + string.Join(", ", imported.Keys.Except(matched)));
                var authored = PrefabUtility.SaveAsPrefabAsset(instance, folder + "character.prefab");
                var definition = AssetDatabase.LoadAssetAtPath<ModernCharacterAsset>(folder + "Character.asset");
                if (definition == null)
                {
                    definition = ScriptableObject.CreateInstance<ModernCharacterAsset>();
                    if (MonoScript.FromScriptableObject(definition) != script)
                    {
                        UnityEngine.Object.DestroyImmediate(definition);
                        throw new InvalidOperationException("Character script association is missing; refusing to overwrite " + folder + "Character.asset");
                    }
                    AssetDatabase.CreateAsset(definition, folder + "Character.asset");
                }
                definition.name = name;
                definition.model = authored;
                definition.clips = AssetDatabase.LoadAllAssetsAtPath(path).OfType<AnimationClip>()
                    .Where(c => manifest.clips.Any(wanted => wanted.name == c.name)).ToArray();
                if (definition.clips.Length != manifest.clips.Length) throw new InvalidOperationException("Imported clip set differs from manifest");
                definition.forwardYaw = 180;
                definition.walkSpeed = manifest.walkSpeed > 0 ? manifest.walkSpeed : walkSpeed;
                definition.runSpeed = manifest.runSpeed;
                definition.hitOverlayBones = manifest.hitOverlayBones ?? Array.Empty<string>();
                if (name == "Villager" && (!float.IsFinite(definition.walkSpeed) || definition.walkSpeed <= 0 ||
                    !float.IsFinite(definition.runSpeed) || definition.runSpeed <= definition.walkSpeed))
                    throw new InvalidOperationException("Villager manifest requires measured Walk and Run travel speeds");
                definition.height = manifest.height;
                if (name == "Villager") definition.ResolveHitOverlayBones(authored.transform);
                EditorUtility.SetDirty(definition);
            }
            finally { UnityEngine.Object.DestroyImmediate(instance); }
        }

        private static string SafeMaterialName(string name) => string.Concat(name.Select(c => char.IsLetterOrDigit(c) ? c : '_'));

        private static ImportedMaps ImportMaps(string folder, MaterialMaps maps, string suffix)
        {
            var albedo = AssetDatabase.LoadAssetAtPath<Texture2D>(folder + maps.albedo);
            var normal = string.IsNullOrWhiteSpace(maps.normal) ? null : AssetDatabase.LoadAssetAtPath<Texture2D>(folder + maps.normal);
            var orm = AssetDatabase.LoadAssetAtPath<Texture2D>(folder + maps.orm);
            if (albedo == null || orm == null || !string.IsNullOrWhiteSpace(maps.normal) && normal == null)
                throw new InvalidOperationException("Declared character PBR maps are absent: " + maps.name);
            var maskPath = folder + suffix + "metallic_occlusion_smoothness.png";
            var mask = new Texture2D(orm.width, orm.height, TextureFormat.RGBA32, false, true);
            try
            {
                var colors = orm.GetPixels32();
                for (var i = 0; i < colors.Length; i++)
                    colors[i] = new Color32(colors[i].b, colors[i].r, 0, (byte)(255 - colors[i].g));
                mask.SetPixels32(colors); mask.Apply();
                File.WriteAllBytes(maskPath, mask.EncodeToPNG());
            }
            finally { UnityEngine.Object.DestroyImmediate(mask); }
            AssetDatabase.ImportAsset(maskPath, ImportAssetOptions.ForceSynchronousImport);
            var maskImport = (TextureImporter)AssetImporter.GetAtPath(maskPath);
            maskImport.sRGBTexture = false; maskImport.mipmapEnabled = true;
            maskImport.textureCompression = TextureImporterCompression.CompressedHQ;
            maskImport.SaveAndReimport();
            return new ImportedMaps { albedo = albedo, normal = normal, packed = AssetDatabase.LoadAssetAtPath<Texture2D>(maskPath) };
        }

        public static void Probe()
        {
            var output = Environment.GetEnvironmentVariable("MC_CHARACTER_IMPORT_REPORT");
            if (string.IsNullOrEmpty(output)) throw new ArgumentException("Set MC_CHARACTER_IMPORT_REPORT");
            var reports = Directory.GetFiles(Folder, "*.fbx", SearchOption.AllDirectories)
                .Select(Inspect).ToArray();
            File.WriteAllText(output, JsonUtility.ToJson(new Report { models = reports }, true));
            Debug.Log("MC_CHARACTER_IMPORT_PROBE " + output);
        }

        private static ModelReport Inspect(string path)
        {
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceSynchronousImport);
            var model = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (model == null) throw new InvalidOperationException("Missing model " + path);
            var importer = (ModelImporter)AssetImporter.GetAtPath(path);
            var instance = UnityEngine.Object.Instantiate(model);
            try
            {
                var renderers = instance.GetComponentsInChildren<Renderer>(true);
                var bounds = renderers[0].bounds;
                foreach (var renderer in renderers.Skip(1)) bounds.Encapsulate(renderer.bounds);
                var transforms = instance.GetComponentsInChildren<Transform>(true);
                return new ModelReport {
                    path = path, scale = instance.transform.localScale,
                    boundsCenter = bounds.center, boundsSize = bounds.size,
                    clips = AssetDatabase.LoadAllAssetsAtPath(path).OfType<AnimationClip>()
                        .Where(clip => !clip.name.StartsWith("__preview__", StringComparison.Ordinal))
                        .Select(clip => new ClipReport { name = clip.name, length = clip.length,
                            frameRate = clip.frameRate, curves = AnimationUtility.GetCurveBindings(clip).Length,
                            shapes = AnimationUtility.GetCurveBindings(clip).Where(binding => binding.propertyName.StartsWith("blendShape.", StringComparison.Ordinal))
                                .Select(binding => { var keys = AnimationUtility.GetEditorCurve(clip, binding).keys; return new ShapeReport {
                                    path = binding.path, property = binding.propertyName,
                                    minimum = keys.Min(key => key.value), maximum = keys.Max(key => key.value) }; }).ToArray()
                        }).ToArray(),
                    takes = importer.defaultClipAnimations.Select(clip => new TakeReport {
                        name = clip.name, take = clip.takeName, first = clip.firstFrame, last = clip.lastFrame }).ToArray(),
                    bones = transforms.Where(t => t.name == "Head" || t.name == "headfront" ||
                        t.name == "Hips" || t.name.Contains("hoof") || t.name.Contains("skull"))
                        .Select(t => new BoneReport { name = t.name, position = t.position }).ToArray(),
                    materials = renderers.SelectMany(renderer => renderer.sharedMaterials).Where(m => m != null)
                        .Distinct().Select(material => new MaterialReport { name = material.name,
                            shader = material.shader.name, texture = material.mainTexture == null ? "" :
                                material.mainTexture.name }).ToArray(),
                    assets = AssetDatabase.LoadAllAssetsAtPath(path).Select(asset => asset.GetType().Name + ":" + asset.name).ToArray()
                };
            }
            finally { UnityEngine.Object.DestroyImmediate(instance); }
        }

        [Serializable] private sealed class Report { public ModelReport[] models; }
        [Serializable] private sealed class ModelReport {
            public string path; public Vector3 scale, boundsCenter, boundsSize;
            public ClipReport[] clips; public TakeReport[] takes; public BoneReport[] bones;
            public MaterialReport[] materials; public string[] assets;
        }
        [Serializable] private sealed class ClipReport { public string name; public float length, frameRate; public int curves; public ShapeReport[] shapes; }
        [Serializable] private sealed class ShapeReport { public string path, property; public float minimum, maximum; }
        [Serializable] private sealed class TakeReport { public string name, take; public float first, last; }
        [Serializable] private sealed class BoneReport { public string name; public Vector3 position; }
        [Serializable] private sealed class MaterialReport { public string name, shader, texture; }
        [Serializable] private sealed class Manifest { public float height, walkSpeed, runSpeed, minBoneWeight; public string[] hitOverlayBones; public ClipRange[] clips; public MaterialMaps material; public MaterialMaps[] materials; }
        private sealed class ImportedMaps { internal Texture2D albedo, normal, packed; }
        [Serializable] private sealed class ClipRange { public string name; public float firstFrame, lastFrame; public bool loop; }
        [Serializable] private sealed class MaterialMaps { public string name, albedo, normal, orm; }
    }
}
