using System;
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

        private void OnPreprocessModel()
        {
            if (!assetPath.StartsWith(Folder, StringComparison.Ordinal)) return;
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
            PrepareCharacter("Goat", .14844f);
            PrepareCharacter("Archer", 1.65f);
            AssetDatabase.SaveAssets();
            Probe();
        }

        private static void PrepareCharacter(string name, float walkSpeed)
        {
            var folder = Folder + name + "/";
            var path = folder + name + ".fbx";
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(folder + name + ".clips.json"));
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate | ImportAssetOptions.ForceSynchronousImport);
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (prefab == null) throw new InvalidOperationException("Missing character FBX " + path);
            foreach (var texture in Directory.GetFiles(folder + "textures"))
            {
                if (texture.EndsWith(".meta", StringComparison.Ordinal)) continue;
                var importer = (TextureImporter)AssetImporter.GetAtPath(texture);
                importer.textureType = texture.Contains("normal") ? TextureImporterType.NormalMap : TextureImporterType.Default;
                importer.sRGBTexture = texture.Contains("albedo");
                importer.mipmapEnabled = true;
                importer.maxTextureSize = 2048;
                importer.anisoLevel = 4;
                importer.isReadable = texture.Contains("orm");
                importer.textureCompression = texture.Contains("orm") ? TextureImporterCompression.Uncompressed : TextureImporterCompression.CompressedHQ;
                importer.SaveAndReimport();
            }
            var albedo = AssetDatabase.LoadAssetAtPath<Texture2D>(folder + manifest.material.albedo);
            var normal = AssetDatabase.LoadAssetAtPath<Texture2D>(folder + manifest.material.normal);
            var orm = AssetDatabase.LoadAssetAtPath<Texture2D>(folder + manifest.material.orm);
            if (albedo == null || normal == null || orm == null) throw new InvalidOperationException("Character PBR maps are absent");
            var maskPath = folder + "metallic_occlusion_smoothness.png";
            var mask = new Texture2D(orm.width, orm.height, TextureFormat.RGBA32, false, true);
            var colors = orm.GetPixels32();
            for (var i = 0; i < colors.Length; i++)
                colors[i] = new Color32(colors[i].b, colors[i].r, 0, (byte)(255 - colors[i].g));
            mask.SetPixels32(colors); mask.Apply();
            File.WriteAllBytes(maskPath, mask.EncodeToPNG());
            UnityEngine.Object.DestroyImmediate(mask);
            AssetDatabase.ImportAsset(maskPath, ImportAssetOptions.ForceSynchronousImport);
            var maskImport = (TextureImporter)AssetImporter.GetAtPath(maskPath);
            maskImport.sRGBTexture = false; maskImport.mipmapEnabled = true;
            maskImport.textureCompression = TextureImporterCompression.CompressedHQ;
            maskImport.SaveAndReimport();
            var packed = AssetDatabase.LoadAssetAtPath<Texture2D>(maskPath);
            var instance = UnityEngine.Object.Instantiate(prefab);
            try
            {
                Directory.CreateDirectory(folder + "materials");
                foreach (var renderer in instance.GetComponentsInChildren<Renderer>(true))
                {
                    renderer.sharedMaterials = renderer.sharedMaterials.Select(source => {
                        var materialPath = folder + "materials/" + string.Concat(source.name.Select(c => char.IsLetterOrDigit(c) ? c : '_')) + ".mat";
                        var material = AssetDatabase.LoadAssetAtPath<Material>(materialPath);
                        if (material == null) { material = new Material(Shader.Find("Universal Render Pipeline/Lit")); AssetDatabase.CreateAsset(material, materialPath); }
                        material.SetColor("_BaseColor", source.color);
                        if (source.name == manifest.material.name)
                        {
                            material.SetTexture("_BaseMap", albedo); material.SetColor("_BaseColor", Color.white);
                            material.SetTexture("_BumpMap", normal); material.EnableKeyword("_NORMALMAP");
                            material.SetTexture("_MetallicGlossMap", packed); material.EnableKeyword("_METALLICSPECGLOSSMAP");
                            material.SetTexture("_OcclusionMap", packed);
                            material.SetFloat("_Smoothness", 1); material.SetFloat("_Metallic", 1);
                        }
                        else material.SetFloat("_Smoothness", .28f);
                        EditorUtility.SetDirty(material);
                        return material;
                    }).ToArray();
                    renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.On;
                }
                var authored = PrefabUtility.SaveAsPrefabAsset(instance, folder + "character.prefab");
                var definition = AssetDatabase.LoadAssetAtPath<ModernCharacterAsset>(folder + "Character.asset");
                if (definition == null) { definition = ScriptableObject.CreateInstance<ModernCharacterAsset>(); AssetDatabase.CreateAsset(definition, folder + "Character.asset"); }
                definition.name = name;
                definition.model = authored;
                definition.clips = AssetDatabase.LoadAllAssetsAtPath(path).OfType<AnimationClip>()
                    .Where(c => manifest.clips.Any(wanted => wanted.name == c.name)).ToArray();
                if (definition.clips.Length != manifest.clips.Length) throw new InvalidOperationException("Imported clip set differs from manifest");
                definition.forwardYaw = 180;
                definition.walkSpeed = walkSpeed;
                definition.height = manifest.height;
                EditorUtility.SetDirty(definition);
            }
            finally { UnityEngine.Object.DestroyImmediate(instance); }
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
        [Serializable] private sealed class Manifest { public float height; public ClipRange[] clips; public MaterialMaps material; }
        [Serializable] private sealed class ClipRange { public string name; public float firstFrame, lastFrame; public bool loop; }
        [Serializable] private sealed class MaterialMaps { public string name, albedo, normal, orm; }
    }
}
