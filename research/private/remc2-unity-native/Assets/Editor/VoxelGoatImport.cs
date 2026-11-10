using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using MagicCarpet.Native.Voxels;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Editor
{
    // Owner-local output; source geometry, material and skin come from the approved prefab.
    public static class VoxelGoatImport
    {
        private sealed class Cell
        {
            internal float distance = float.PositiveInfinity;
            internal Vector3 center;
            internal Color32 color;
            internal BoneWeight weight;
        }

        [MenuItem("Magic Carpet/Prepare voxel characters")]
        public static void Prepare()
        {
            PrepareCharacter("Goat", new[] { 24, 40, 64, 96 });
            PrepareCharacter("Archer", new[] { 64, 96 });
            foreach (var binding in ModernVillagerAppearances.All) PrepareCharacter(binding.Resource, new[] { 64, 96 });
            foreach (var binding in ModernCreatureAssets.All) PrepareCharacter(binding.Resource, new[] { 64, 96 });
            AssetDatabase.SaveAssets();
        }

        private static void PrepareCharacter(string name, int[] resolutions)
        {
            var character = Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character");
            if (character == null || character.model == null || character.height <= 0)
                throw new InvalidOperationException("The approved " + name + " character must be imported first");
            var folder = "Assets/Resources/ModernCharacters/" + name + "/Voxels";
            Directory.CreateDirectory(folder);
            AssetDatabase.Refresh();
            var fingerprint = AssetDatabase.GetAssetDependencyHash(AssetDatabase.GetAssetPath(character.model)).ToString() +
                "-" + character.height.ToString("R", System.Globalization.CultureInfo.InvariantCulture) + "-cpu-albedo1";
            var instance = UnityEngine.Object.Instantiate(character.model);
            try
            {
                var skins = instance.GetComponentsInChildren<SkinnedMeshRenderer>(true);
                var renderer = name == "Archer" ? skins.Single(skin => skin.name == "char1") : skins.Single();
                foreach (var resolution in resolutions)
                {
                    // Multipart creatures render many copies of their 64-cell
                    // skins. Retain every occupied cell for fracture, but omit
                    // coincident interior faces with identical skin transforms.
                    // Accepted legacy character meshes keep their current form.
                    var optimize = resolution == 96 || resolution == 64 && ModernCreatureAssets.ContainsResource(name);
                    var path = $"{folder}/{name}{resolution}.asset";
                    var existing = AssetDatabase.LoadAssetAtPath<VoxelGoatAsset>(path);
                    if (existing != null && (ModernVillagerAppearances.ContainsResource(name) || !string.IsNullOrEmpty(existing.sourceFingerprint)) &&
                        existing.sourceFingerprint != fingerprint)
                    {
                        // Preserve losing derived output outside Resources; it must not
                        // silently survive a changed source mesh, rig or fabric map.
                        var history = "Assets/GeneratedCharacterHistory/" + name;
                        Directory.CreateDirectory(history);
                        AssetDatabase.Refresh();
                        var archived = AssetDatabase.GenerateUniqueAssetPath(history + "/" + name + resolution + ".asset");
                        var error = AssetDatabase.MoveAsset(path, archived);
                        if (!string.IsNullOrEmpty(error)) throw new InvalidOperationException("Cannot preserve prior voxel asset: " + error);
                        existing = null;
                    }
                    if (existing != null)
                    {
                        if (existing.sourceMesh != renderer.sharedMesh || existing.mesh == null || existing.resolution != resolution)
                            throw new InvalidOperationException("Existing generated asset differs from approved source: " + path);
                        if (optimize && (existing.meshFormat == 0 ||
                            existing.meshFormat == 1 && existing.mesh.vertexCount != CountFaceVertices(existing.cellFaceMasks)))
                            Optimize(existing);
                        else Debug.Log("Retaining existing voxel character " + path);
                        continue;
                    }
                    var result = Build(character, renderer, resolution);
                    result.sourceFingerprint = fingerprint;
                    result.name = name + resolution;
                    AssetDatabase.CreateAsset(result, path);
                    AssetDatabase.AddObjectToAsset(result.mesh, result);
                    if (optimize) Optimize(result);
                    EditorUtility.SetDirty(result);
                    Debug.Log($"Voxel {name} {resolution}: {result.cellColors.Length} cells, {result.cellSize} local units");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(instance); }
        }

        private static VoxelGoatAsset Build(ModernCharacterAsset character, SkinnedMeshRenderer renderer, int resolution)
        {
            var source = renderer.sharedMesh;
            if (source == null || !source.isReadable || source.boneWeights.Length != source.vertexCount || source.uv.Length != source.vertexCount)
                throw new InvalidOperationException("Character mesh needs readable vertices, UVs and bone weights");
            var relativeY = renderer.transform.InverseTransformVector(instanceUp(renderer.transform.root)).magnitude;
            var size = character.height * relativeY / resolution;
            var vertices = source.vertices; var uv = source.uv; var weights = source.boneWeights;
            var cells = new Dictionary<Vector3Int, Cell>();
            var copies = new List<Texture2D>();
            try
            {
                var materials = renderer.sharedMaterials;
                for (var part = 0; part < source.subMeshCount; part++)
                {
                    var material = materials[Mathf.Min(part, materials.Length - 1)];
                    var texture = material.HasProperty("_BaseMap") ? material.GetTexture("_BaseMap") : material.mainTexture;
                    var readable = texture != null ? ReadTexture(texture) : null;
                    if (readable != null) copies.Add(readable);
                    var tint = material.HasProperty("_BaseColor") ? material.GetColor("_BaseColor") : material.color;
                    var indices = source.GetTriangles(part);
                    for (var t = 0; t < indices.Length; t += 3)
                    {
                        var ia = indices[t]; var ib = indices[t + 1]; var ic = indices[t + 2];
                        var a = vertices[ia]; var b = vertices[ib]; var c = vertices[ic];
                        var min = Vector3.Min(a, Vector3.Min(b, c)) / size - Vector3.one;
                        var max = Vector3.Max(a, Vector3.Max(b, c)) / size + Vector3.one;
                        for (var x = Mathf.FloorToInt(min.x); x <= Mathf.FloorToInt(max.x); x++)
                        for (var y = Mathf.FloorToInt(min.y); y <= Mathf.FloorToInt(max.y); y++)
                        for (var z = Mathf.FloorToInt(min.z); z <= Mathf.FloorToInt(max.z); z++)
                        {
                            var key = new Vector3Int(x, y, z);
                            var center = (new Vector3(x, y, z) + Vector3.one * .5f) * size;
                            var bary = ClosestBarycentric(center, a, b, c);
                            var closest = a * bary.x + b * bary.y + c * bary.z;
                            var distance = (center - closest).sqrMagnitude;
                            if (distance > size * size * .75001f) continue;
                            if (!cells.TryGetValue(key, out var cell) || cell == null)
                            {
                                cell = new Cell { center = center };
                                cells[key] = cell;
                            }
                            if (distance >= cell.distance) continue;
                            cell.distance = distance;
                            var texcoord = uv[ia] * bary.x + uv[ib] * bary.y + uv[ic] * bary.z;
                            cell.color = (readable != null ? readable.GetPixelBilinear(texcoord.x, texcoord.y) : Color.white) * tint;
                            cell.weight = Blend(weights[ia], weights[ib], weights[ic], bary);
                        }
                    }
                }
                if (cells.Count == 0) throw new InvalidOperationException("Voxelization produced no cells");
                var ordered = cells.OrderBy(pair => pair.Key.x).ThenBy(pair => pair.Key.y).ThenBy(pair => pair.Key.z).Select(pair => pair.Value).ToArray();
                var output = ScriptableObject.CreateInstance<VoxelGoatAsset>();
                output.name = "Goat" + resolution; output.sourceMesh = source; output.cellSize = size; output.resolution = resolution;
                output.cellColors = ordered.Select(cell => cell.color).ToArray();
                output.cellCenters = ordered.Select(cell => cell.center).ToArray();
                output.cellWeights = ordered.Select(cell => cell.weight).ToArray();
                output.mesh = MakeMesh(source, ordered, size);
                return output;
            }
            finally { foreach (var copy in copies) UnityEngine.Object.DestroyImmediate(copy); }
        }

        // The authored model height is in prefab-root units; convert to skin-local units.
        private static Vector3 instanceUp(Transform root) => root.TransformVector(Vector3.up);

        private static Texture2D ReadTexture(Texture texture)
        {
            // Graphics.Blit/ReadPixels silently produce grey under -nographics.
            // Read the authored sRGB image on the CPU so batch conversion and
            // interactive conversion sample the same texels without a GPU.
            var path = AssetDatabase.GetAssetPath(texture);
            var result = new Texture2D(2, 2, TextureFormat.RGBA32, false, false);
            try
            {
                if (string.IsNullOrEmpty(path) || !File.Exists(path) ||
                    !ImageConversion.LoadImage(result, File.ReadAllBytes(path), false))
                    throw new InvalidOperationException("Voxel albedo must be a decodable authored image: " + path);
                result.wrapMode = texture.wrapMode;
                return result;
            }
            catch { UnityEngine.Object.DestroyImmediate(result); throw; }
        }

        private static BoneWeight Blend(BoneWeight a, BoneWeight b, BoneWeight c, Vector3 bary)
        {
            var result = new Dictionary<int, float>();
            void Add(BoneWeight w, float k)
            {
                void One(int index, float value) { if (value > 0) result[index] = (result.TryGetValue(index, out var old) ? old : 0) + value * k; }
                One(w.boneIndex0, w.weight0); One(w.boneIndex1, w.weight1); One(w.boneIndex2, w.weight2); One(w.boneIndex3, w.weight3);
            }
            Add(a, bary.x); Add(b, bary.y); Add(c, bary.z);
            var top = result.OrderByDescending(p => p.Value).Take(4).ToArray();
            var sum = top.Sum(p => p.Value);
            if (sum <= 0) throw new InvalidOperationException("Source triangle has no skin weights");
            var w = new BoneWeight();
            for (var i = 0; i < top.Length; i++)
            {
                var v = top[i].Value / sum; var index = top[i].Key;
                if (i == 0) { w.boneIndex0 = index; w.weight0 = v; }
                else if (i == 1) { w.boneIndex1 = index; w.weight1 = v; }
                else if (i == 2) { w.boneIndex2 = index; w.weight2 = v; }
                else { w.boneIndex3 = index; w.weight3 = v; }
            }
            return w;
        }

        // Closest point on a triangle, returned as barycentric coordinates.
        private static Vector3 ClosestBarycentric(Vector3 p, Vector3 a, Vector3 b, Vector3 c)
        {
            var ab = b - a; var ac = c - a; var ap = p - a;
            if (Vector3.Cross(ab, ac).sqrMagnitude < 1e-30f)
            {
                // Degenerate FBX triangles still contribute their longest edge.
                var bc = c - b;
                if (ab.sqrMagnitude >= ac.sqrMagnitude && ab.sqrMagnitude >= bc.sqrMagnitude && ab.sqrMagnitude > 0)
                { var t = Mathf.Clamp01(Vector3.Dot(ap, ab) / ab.sqrMagnitude); return new Vector3(1 - t, t, 0); }
                if (ac.sqrMagnitude >= bc.sqrMagnitude && ac.sqrMagnitude > 0)
                { var t = Mathf.Clamp01(Vector3.Dot(ap, ac) / ac.sqrMagnitude); return new Vector3(1 - t, 0, t); }
                if (bc.sqrMagnitude > 0)
                { var t = Mathf.Clamp01(Vector3.Dot(p - b, bc) / bc.sqrMagnitude); return new Vector3(0, 1 - t, t); }
                return new Vector3(1, 0, 0);
            }
            var d1 = Vector3.Dot(ab, ap); var d2 = Vector3.Dot(ac, ap);
            if (d1 <= 0 && d2 <= 0) return new Vector3(1, 0, 0);
            var bp = p - b; var d3 = Vector3.Dot(ab, bp); var d4 = Vector3.Dot(ac, bp);
            if (d3 >= 0 && d4 <= d3) return new Vector3(0, 1, 0);
            var vc = d1 * d4 - d3 * d2;
            if (vc <= 0 && d1 >= 0 && d3 <= 0) { var v = d1 / (d1 - d3); return new Vector3(1 - v, v, 0); }
            var cp = p - c; var d5 = Vector3.Dot(ab, cp); var d6 = Vector3.Dot(ac, cp);
            if (d6 >= 0 && d5 <= d6) return new Vector3(0, 0, 1);
            var vb = d5 * d2 - d1 * d6;
            if (vb <= 0 && d2 >= 0 && d6 <= 0) { var w = d2 / (d2 - d6); return new Vector3(1 - w, 0, w); }
            var va = d3 * d6 - d5 * d4;
            if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) { var w = (d4 - d3) / (d4 - d3 + d5 - d6); return new Vector3(0, 1 - w, w); }
            var denominator = va + vb + vc;
            if (Mathf.Abs(denominator) < 1e-20f) return new Vector3(1, 0, 0);
            var v2 = vb / denominator; var w2 = vc / denominator;
            return new Vector3(1 - v2 - w2, v2, w2);
        }

        private static int CountFaceVertices(byte[] masks)
        {
            if (masks == null) return -1;
            var faces = 0;
            foreach (var mask in masks)
                for (var face = 0; face < 6; face++) if ((mask & (1 << face)) != 0) faces++;
            return faces * 4;
        }

        // Only erase paired faces whose complete skin transform is identical.
        // The adjoining quads then remain coincident under every rig pose, so
        // no hole can open even at a moving joint. Different weights keep both.
        private static void Optimize(VoxelGoatAsset asset)
        {
            if (asset.cellCenters == null || asset.cellWeights == null || asset.cellColors == null ||
                asset.cellCenters.Length != asset.cellColors.Length || asset.cellWeights.Length != asset.cellColors.Length)
                throw new InvalidOperationException("Cannot optimize a legacy character without center and weight metadata");
            var size = asset.cellSize;
            var count = asset.cellCenters.Length;
            var lookup = new Dictionary<Vector3Int, int>(count);
            Vector3Int Key(Vector3 p) => new Vector3Int(Mathf.RoundToInt(p.x / size - .5f),
                Mathf.RoundToInt(p.y / size - .5f), Mathf.RoundToInt(p.z / size - .5f));
            for (var i = 0; i < count; i++) lookup.Add(Key(asset.cellCenters[i]), i);
            var directions = new[] { Vector3Int.right, Vector3Int.left, Vector3Int.up, Vector3Int.down,
                new Vector3Int(0, 0, 1), new Vector3Int(0, 0, -1) };
            var faceNormals = new[] { Vector3.right, Vector3.left, Vector3.up, Vector3.down, Vector3.forward, Vector3.back };
            var axesU = new[] { Vector3.back, Vector3.forward, Vector3.right, Vector3.right, Vector3.right, Vector3.left };
            var axesV = new[] { Vector3.up, Vector3.up, Vector3.back, Vector3.forward, Vector3.up, Vector3.up };
            var masks = new byte[count];
            var faces = 0;
            for (var i = 0; i < count; i++)
            {
                var key = Key(asset.cellCenters[i]);
                for (var face = 0; face < 6; face++)
                {
                    if (lookup.TryGetValue(key + directions[face], out var neighbor) && asset.cellWeights[i].Equals(asset.cellWeights[neighbor])) continue;
                    masks[i] |= (byte)(1 << face); faces++;
                }
            }
            if (faces == 0) throw new InvalidOperationException("Voxel optimization produced no visible faces");
            var verts = new Vector3[faces * 4]; var normals = new Vector3[verts.Length];
            var colors = new Color32[verts.Length]; var weights = new BoneWeight[verts.Length];
            var indices = new int[faces * 6]; var n = 0;
            for (var i = 0; i < count; i++)
            for (var face = 0; face < 6; face++)
            {
                if ((masks[i] & (1 << face)) == 0) continue;
                var start = n * 4; var p = asset.cellCenters[i] + faceNormals[face] * size * .5f;
                var u = axesU[face] * size * .5f; var v = axesV[face] * size * .5f;
                verts[start] = p - u - v; verts[start + 1] = p + u - v;
                verts[start + 2] = p + u + v; verts[start + 3] = p - u + v;
                for (var k = 0; k < 4; k++) { normals[start + k] = faceNormals[face]; colors[start + k] = asset.cellColors[i]; weights[start + k] = asset.cellWeights[i]; }
                var t = n * 6;
                indices[t] = start; indices[t + 1] = start + 1; indices[t + 2] = start + 2;
                indices[t + 3] = start; indices[t + 4] = start + 2; indices[t + 5] = start + 3;
                n++;
            }
            // CopySerialized can silently leave a Mesh unchanged on some
            // existing Unity subassets (observed on Goat96). Assign its data
            // directly, preserving the subasset identity and existing references.
            var mesh = asset.mesh;
            var before = mesh.vertexCount;
            var poses = mesh.bindposes;
            var bounds = mesh.bounds;
            mesh.Clear();
            mesh.indexFormat = IndexFormat.UInt32;
            mesh.vertices = verts; mesh.normals = normals; mesh.colors32 = colors; mesh.boneWeights = weights;
            mesh.bindposes = poses; mesh.triangles = indices; mesh.bounds = bounds;
            if (mesh.vertexCount != faces * 4 || mesh.triangles.Length != faces * 6)
                throw new InvalidOperationException("Voxel mesh update did not apply completely");
            asset.cellFaceMasks = masks; asset.meshFormat = 1;
            EditorUtility.SetDirty(mesh); EditorUtility.SetDirty(asset);
            Debug.Log($"Voxel character {asset.name} exact-weight faces: {before} -> {mesh.vertexCount} vertices ({faces} faces)");
        }

        private static Mesh MakeMesh(Mesh source, Cell[] cells, float size)
        {
            var verts = new Vector3[cells.Length * 24]; var normals = new Vector3[verts.Length];
            var colors = new Color32[verts.Length]; var weights = new BoneWeight[verts.Length];
            var indices = new int[cells.Length * 36];
            var faceNormals = new[] { Vector3.right, Vector3.left, Vector3.up, Vector3.down, Vector3.forward, Vector3.back };
            var axesU = new[] { Vector3.back, Vector3.forward, Vector3.right, Vector3.right, Vector3.right, Vector3.left };
            var axesV = new[] { Vector3.up, Vector3.up, Vector3.back, Vector3.forward, Vector3.up, Vector3.up };
            for (var i = 0; i < cells.Length; i++)
            for (var f = 0; f < 6; f++)
            {
                var start = i * 24 + f * 4; var face = cells[i].center + faceNormals[f] * size * .5f;
                var u = axesU[f] * size * .5f; var v = axesV[f] * size * .5f;
                verts[start] = face - u - v; verts[start + 1] = face + u - v;
                verts[start + 2] = face + u + v; verts[start + 3] = face - u + v;
                for (var k = 0; k < 4; k++) { normals[start + k] = faceNormals[f]; colors[start + k] = cells[i].color; weights[start + k] = cells[i].weight; }
                var ti = i * 36 + f * 6;
                indices[ti] = start; indices[ti + 1] = start + 1; indices[ti + 2] = start + 2;
                indices[ti + 3] = start; indices[ti + 4] = start + 2; indices[ti + 5] = start + 3;
            }
            var mesh = new Mesh { name = "Goat voxel skin", indexFormat = IndexFormat.UInt32 };
            mesh.vertices = verts; mesh.normals = normals; mesh.colors32 = colors; mesh.boneWeights = weights;
            mesh.bindposes = source.bindposes; mesh.triangles = indices;
            mesh.bounds = source.bounds; return mesh;
        }
    }
}
