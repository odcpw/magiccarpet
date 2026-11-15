using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Presentation only: these small trees and rocks are not authored, collidable scenery.
    // The terrain remains the sole authority for height and water.
    internal sealed class ModernVegetation : IDisposable
    {
        private const float Cell = 4f;
        private const int WrappedCells = 256;
        private const float DetailRadius = 115f;
        private const float LandmarkRadius = 200f;
        private const int CellsPerChunk = 8;
        private const float ChunkSize = Cell * CellsPerChunk;
        // The scanned mesh is useful only while its silhouette occupies enough
        // pixels to show its geometry. Whole groups use conservative bounds;
        // only groups intersecting the transition band need per-instance work.
        private const float DetailPixels = 48f;
        private const float FadeHalfWidth = 8f;
        private const float ImpostorPixels = 16f;
        private const float ImpostorFadeHalfWidth = 4f;
        private static readonly string[] IntermediateResourceNames =
        {
            "grass_mid", "fern_mid", "shrub_mid", "tree_trunk_mid", "rock_mid", "tree_branches_mid", "tree_leaves_mid"
        };
        // Quality reference only: preserve placement, view ranges and shadow
        // routing while rendering the full scanned meshes at every distance.
        private static readonly bool FullVegetationReference =
            Array.IndexOf(Environment.GetCommandLineArgs(), "-mc-modern-full-vegetation") >= 0;
        private readonly ModernTerrainMesh terrain;
        private readonly Material[] materials;
        private readonly Material[] distantMaterials;
        private readonly Mesh[] meshes;
        private readonly Mesh[] intermediateMeshes;
        private readonly Mesh[] distantMeshes;
        private readonly long[] detailTriangleCounts;
        private readonly long[] intermediateTriangleCounts;
        private readonly long[] distantTriangleCounts;
        private readonly Dictionary<Vector2Int, Chunk> chunks = new Dictionary<Vector2Int, Chunk>();
        private readonly List<Vector2Int> expired = new List<Vector2Int>();
        private readonly Plane[] frustum = new Plane[6];
        private readonly ViewVersion mainView = new ViewVersion();
        private readonly ViewVersion reflectedView = new ViewVersion();
        private readonly BatchAccumulator mainBatches;
        private readonly BatchAccumulator reflectedBatches;
        private Camera mainSubmissionCamera;
        private Camera reflectedSubmissionCamera;
        private Bounds treeLocalBounds;
        private int centerX = int.MinValue;
        private int centerZ = int.MinValue;
        private ulong terrainRevision = ulong.MaxValue;
        private bool dirty = true;
        private bool residencyChanged;
        private bool disposed;
        // Sandbox architecture owns its footprint. Exclusion is evaluated only
        // when retained chunks are built, never as an extra per-frame draw scan.
        internal Func<float,float,bool> ExcludedFootprint { get; set; }

        internal void InvalidateFootprint(Bounds area)
        {
            // Rebuild only retained chunks affected by a local excavation.
            // The updated exclusion predicate removes uprooted instances.
            var keys=chunks.Keys.Where(key=>key.x*ChunkSize<=area.max.x && (key.x+1)*ChunkSize>=area.min.x &&
                key.y*ChunkSize<=area.max.z && (key.y+1)*ChunkSize>=area.min.z).ToArray();
            foreach(var key in keys)chunks.Remove(key);
            residencyChanged=true;
        }

        internal long SubmittedTriangleCount { get; private set; }
        internal long ShadowOnlyTriangleCount { get; private set; }
        internal int DrawCount { get; private set; }
        internal int DetailInstanceCount { get; private set; }
        internal int IntermediateInstanceCount { get; private set; }
        internal int DistantInstanceCount { get; private set; }

        private sealed class Chunk
        {
            internal readonly List<Matrix4x4>[] Instances = new List<Matrix4x4>[7];
            internal readonly List<LodInstance>[] LodInstances = new List<LodInstance>[7];
            internal readonly Bounds[] LodCenters = new Bounds[7];
            internal readonly float[] MinimumDiameter = new float[7];
            internal readonly float[] MaximumDiameter = new float[7];
            internal readonly LodBucket[] Main = new LodBucket[7];
            internal readonly LodBucket[] Reflection = new LodBucket[7];
            internal bool HasDetail;
            internal bool HasBounds;
            internal Bounds Bounds;

            internal Chunk()
            {
                for (var i = 0; i < Instances.Length; i++)
                {
                    Instances[i] = new List<Matrix4x4>();
                    LodInstances[i] = new List<LodInstance>();
                    MinimumDiameter[i] = float.PositiveInfinity;
                }
            }
        }

        private struct LodInstance
        {
            internal Vector3 Center;
            internal float Diameter;
        }

        private sealed class LodBucket
        {
            // 1 = all detail, 2 = all intermediate, 3 = all distant, 4 = partition.
            internal int Mode;
            internal int Version = -1;
            internal List<Matrix4x4> Near;
            internal List<Matrix4x4> Intermediate;
            internal List<Matrix4x4> Far;
            internal float[] NearFade;
            internal float[] IntermediateFade;
            internal float[] FarFade;
        }

        // Groups only already-selected instances. Each key has a bounded reusable
        // buffer; a full buffer is submitted immediately and reused.
        internal sealed class BatchAccumulator
        {
            internal const int Capacity = 256;
            private const int SpatialSlotsPerAxis = 4;
            private const float ShadowBinSize = 64f;
            internal readonly BatchBuffer[] Buffers = new BatchBuffer[7 * 3 * 3];
            private readonly BatchBuffer[] shadowBins = new BatchBuffer[7 * 3 * SpatialSlotsPerAxis * SpatialSlotsPerAxis];
            private readonly Action<BatchBuffer> submit;

            internal BatchAccumulator(Action<BatchBuffer> submit)
            {
                this.submit = submit ?? throw new ArgumentNullException(nameof(submit));
                for (var kind = 0; kind < 7; kind++)
                for (var level = 0; level < 3; level++)
                for (var shadowKey = 0; shadowKey < 3; shadowKey++)
                {
                    var shadows = shadowKey == 0 ? ShadowCastingMode.Off :
                        shadowKey == 1 ? ShadowCastingMode.On : ShadowCastingMode.ShadowsOnly;
                    Buffers[(kind * 3 + level) * 3 + shadowKey] = new BatchBuffer(kind, level, shadows);
                }
                for (var kind = 0; kind < 7; kind++)
                for (var level = 0; level < 3; level++)
                for (var slot = 0; slot < SpatialSlotsPerAxis * SpatialSlotsPerAxis; slot++)
                    shadowBins[(kind * 3 + level) * SpatialSlotsPerAxis * SpatialSlotsPerAxis + slot] =
                        new BatchBuffer(kind, level, ShadowCastingMode.On);
            }

            internal void Add(int kind, int level, ShadowCastingMode shadows, List<Matrix4x4> instances, float[] fades)
            {
                if (instances == null) throw new ArgumentNullException(nameof(instances));
                if (fades != null && fades.Length < instances.Count) throw new ArgumentException("Fade array is shorter than the instance list", nameof(fades));
                var shadowKey = shadows == ShadowCastingMode.Off ? 0 : shadows == ShadowCastingMode.On ? 1 :
                    shadows == ShadowCastingMode.ShadowsOnly ? 2 : throw new ArgumentOutOfRangeException(nameof(shadows));
                if (kind < 0 || kind >= 7) throw new ArgumentOutOfRangeException(nameof(kind));
                if (level < 0 || level >= 3) throw new ArgumentOutOfRangeException(nameof(level));
                if (shadows == ShadowCastingMode.On)
                {
                    for (var i = 0; i < instances.Count; i++)
                    {
                        var position = instances[i].GetColumn(3);
                        var binX = Mathf.FloorToInt(position.x / ShadowBinSize);
                        var binZ = Mathf.FloorToInt(position.z / ShadowBinSize);
                        var slot = (binX & (SpatialSlotsPerAxis - 1)) * SpatialSlotsPerAxis +
                            (binZ & (SpatialSlotsPerAxis - 1));
                        var spatial = shadowBins[(kind * 3 + level) * SpatialSlotsPerAxis * SpatialSlotsPerAxis + slot];
                        if (spatial.Instances.Count > 0 && (spatial.BinX != binX || spatial.BinZ != binZ)) Submit(spatial);
                        spatial.BinX = binX;
                        spatial.BinZ = binZ;
                        spatial.Append(instances, i, 1, fades);
                        if (spatial.Instances.Count == Capacity) Submit(spatial);
                    }
                    return;
                }
                var index = (kind * 3 + level) * 3 + shadowKey;
                var buffer = Buffers[index];
                var offset = 0;
                while (offset < instances.Count)
                {
                    var count = Math.Min(Capacity - buffer.Instances.Count, instances.Count - offset);
                    buffer.Append(instances, offset, count, fades);
                    offset += count;
                    if (buffer.Instances.Count == Capacity) Submit(buffer);
                }
            }

            internal void Flush()
            {
                foreach (var buffer in Buffers) if (buffer.Instances.Count > 0) Submit(buffer);
                foreach (var buffer in shadowBins) if (buffer.Instances.Count > 0) Submit(buffer);
            }

            internal void Clear()
            {
                foreach (var buffer in Buffers) buffer.Clear();
                foreach (var buffer in shadowBins) buffer.Clear();
            }

            private void Submit(BatchBuffer buffer)
            {
                submit(buffer);
                buffer.Clear();
            }
        }

        internal sealed class BatchBuffer
        {
            internal readonly int Kind;
            internal readonly int Level;
            internal readonly ShadowCastingMode Shadows;
            internal int BinX;
            internal int BinZ;
            internal readonly List<Matrix4x4> Instances = new List<Matrix4x4>(BatchAccumulator.Capacity);
            internal readonly float[] Fades = new float[BatchAccumulator.Capacity];
            private readonly MaterialPropertyBlock properties = new MaterialPropertyBlock();
            private bool hasOverrides;

            internal BatchBuffer(int kind, int level, ShadowCastingMode shadows)
            { Kind = kind; Level = level; Shadows = shadows; }

            internal void Append(List<Matrix4x4> source, int offset, int count, float[] fades)
            {
                var start = Instances.Count;
                if (offset == 0 && count == source.Count) Instances.AddRange(source);
                else for (var i = 0; i < count; i++) Instances.Add(source[offset + i]);
                if (fades == null) Array.Fill(Fades, Level == 2 ? 0f : -1f, start, count);
                else { Array.Copy(fades, offset, Fades, start, count); hasOverrides = true; }
            }

            internal MaterialPropertyBlock Properties()
            {
                if (!hasOverrides) return null;
                properties.SetFloatArray("_LodThreshold", Fades);
                return properties;
            }

            internal void Clear() { Instances.Clear(); hasOverrides = false; }
        }

        private sealed class ViewVersion
        {
            internal int Version;
            private bool valid;
            private Matrix4x4 view;
            private Matrix4x4 projection;
            private int width, height;

            internal int Update(Camera camera)
            {
                var currentView = camera.worldToCameraMatrix;
                var currentProjection = camera.projectionMatrix;
                if (!valid || view != currentView || projection != currentProjection ||
                    width != camera.pixelWidth || height != camera.pixelHeight)
                {
                    view = currentView; projection = currentProjection;
                    width = camera.pixelWidth; height = camera.pixelHeight;
                    valid = true;
                    unchecked { Version++; }
                }
                return Version;
            }
        }

        internal ModernVegetation(ModernTerrainMesh terrain, bool voxel = false)
        {
            this.terrain = terrain ?? throw new ArgumentNullException(nameof(terrain));
            mainBatches = new BatchAccumulator(SubmitMainBatch);
            reflectedBatches = new BatchAccumulator(SubmitReflectedBatch);
            var shader = Resources.Load<Shader>("ModernVegetation");
            if (shader == null)
                throw new InvalidOperationException("Missing Resources/ModernVegetation shader");
            meshes = new Mesh[7];
            intermediateMeshes = new Mesh[7];
            distantMeshes = new Mesh[7];
            detailTriangleCounts = new long[7];
            intermediateTriangleCounts = new long[7];
            distantTriangleCounts = new long[7];
            materials = new Material[7];
            distantMaterials = new Material[7];
            try
            {
                var treeBounds = voxel ? default : ScannedTreeBounds();
                for (var i = 0; i < meshes.Length; i++)
                    meshes[i] = voxel ? LoadVoxelMesh(i, 0) : LoadSourceMesh(i, treeBounds);
                materials[0] = ScannedMaterial(shader, "grass_medium_01", meshes[0], true);
                materials[1] = ScannedMaterial(shader, "fern_02", meshes[1], true);
                materials[2] = ScannedMaterial(shader, "shrub_02", meshes[2], true);
                materials[3] = ScannedMaterial(shader, "tree_small_02", meshes[3], false);
                materials[4] = ScannedMaterial(shader, "rock_moss_set_01", meshes[4], false);
                materials[5] = ScannedMaterial(shader, "tree_small_02_branch", meshes[5], false);
                materials[5].SetFloat("_WindStrength", 0.03f);
                materials[6] = ScannedMaterial(shader, "tree_small_02_leaves", meshes[6], true);
                for (var i = 0; i < meshes.Length; i++)
                {
                    if (voxel)
                    {
                        intermediateMeshes[i] = LoadVoxelMesh(i, 1);
                        // Surface colors are baked once from source albedo and
                        // alpha; retain the instanced wind/fade shader path.
                        materials[i].SetFloat("_UseTextures", 0f);
                        materials[i].SetFloat("_UseNormalMap", 0f);
                        materials[i].SetFloat("_UseRoughnessMap", 0f);
                        materials[i].SetFloat("_Smoothness", 0f);
                        if (Voxels.VoxelVegetationMesh.UsesAlpha(i))
                        {
                            distantMeshes[i] = MakeImpostorCards(meshes[i]);
                            distantMaterials[i] = ImpostorMaterial(materials[i], meshes[i], "Voxels/" + Voxels.VoxelVegetationMesh.KindName(i));
                        }
                        else distantMeshes[i] = LoadVoxelMesh(i, 2);
                    }
                    else
                    {
                        var intermediate = Resources.Load<Mesh>("ModernNature/" + IntermediateResourceName(i));
                        if (intermediate == null) throw new InvalidOperationException("Missing authored vegetation mesh: " + IntermediateResourceName(i));
                        intermediateMeshes[i] = UnityEngine.Object.Instantiate(intermediate);
                        if (i < 3 || i == 6)
                        {
                            distantMeshes[i] = MakeImpostorCards(meshes[i]);
                            var name = i == 0 ? "grass" : i == 1 ? "fern" : i == 2 ? "shrub" : "tree_leaves";
                            distantMaterials[i] = ImpostorMaterial(materials[i], meshes[i], name);
                        }
                        else if (i == 3 || i == 5)
                            distantMeshes[i] = LoadScannedMesh("tree_small_02_lod", i == 3 ? "tree_trunk" : "tree_branches", 4.5f, treeBounds);
                        else
                            distantMeshes[i] = LoadScannedMesh("rock_moss_set_01_lod", "rock_moss_set_01_rock03", 2.4f);
                    }
                    if (distantMaterials[i] == null)
                        distantMaterials[i] = new Material(materials[i]) { enableInstancing = true, name = materials[i].name + " distant" };
                    // Endpoint batches need no per-instance MaterialPropertyBlock.
                    distantMaterials[i].SetFloat("_LodThreshold", 0f);
                    detailTriangleCounts[i] = (long)meshes[i].GetIndexCount(0) / 3;
                    intermediateTriangleCounts[i] = (long)intermediateMeshes[i].GetIndexCount(0) / 3;
                    distantTriangleCounts[i] = (long)distantMeshes[i].GetIndexCount(0) / 3;
                }
                treeLocalBounds = CombinedTreeBounds();
            }
            catch
            {
                Dispose();
                throw;
            }
        }

        private static Mesh LoadVoxelMesh(int kind, int lod)
        {
            var name = Voxels.VoxelVegetationMesh.ResourceName(kind, lod);
            var mesh = Resources.Load<Mesh>("ModernNature/Voxels/" + name);
            if (mesh == null || mesh.vertexCount == 0 || mesh.GetIndexCount(0) == 0)
                throw new InvalidOperationException("Missing voxel vegetation mesh: " + name);
            return UnityEngine.Object.Instantiate(mesh);
        }

        // Call after the source terrain planes change, even if the camera has not moved.
        internal void Invalidate() => dirty = true;

        internal void Render(Camera camera, bool reflection = false, float resolutionScale = 1f)
        {
            if (!reflection)
            {
                SubmittedTriangleCount = 0;
                ShadowOnlyTriangleCount = 0;
                DrawCount = 0;
                DetailInstanceCount = 0;
                IntermediateInstanceCount = 0;
                DistantInstanceCount = 0;
            }
            if (disposed || camera == null || !SystemInfo.supportsInstancing) return;
            var position = camera.transform.position;
            var forward = camera.transform.forward;
            var pixelsPerUnit = Mathf.Abs(camera.projectionMatrix.m11) * camera.pixelHeight * .5f;
            var orthographic = camera.orthographic;
            var version = (reflection ? reflectedView : mainView).Update(camera);
            RefreshChunks(position);
            GeometryUtility.CalculateFrustumPlanes(camera, frustum);
            var batches = reflection ? reflectedBatches : mainBatches;
            if (reflection) reflectedSubmissionCamera = camera;
            else mainSubmissionCamera = camera;
            try
            {
            foreach (var entry in chunks)
            {
                var distance = DistanceToChunk(position.x, position.z, entry.Key.x, entry.Key.y);
                if (distance > LandmarkRadius) continue;
                var visible = entry.Value.HasBounds && GeometryUtility.TestPlanesAABB(frustum, entry.Value.Bounds);
                if (reflection && !visible) continue;
                for (var kind = 0; kind < meshes.Length; kind++)
                {
                    // Landmark batches are still submitted offscreen: their shadows
                    // can fall into view even when their geometry does not.
                    if (!visible && kind < 3) continue;
                    if (kind < 3 && distance > DetailRadius) continue;
                    var list = entry.Value.Instances[kind];
                    if (list.Count == 0) continue;
                    if (FullVegetationReference)
                    {
                        var referenceShadows = kind >= 3 && !reflection
                            ? (visible ? ShadowCastingMode.On : ShadowCastingMode.ShadowsOnly)
                            : ShadowCastingMode.Off;
                        DrawBatch(batches, kind, list, 0, referenceShadows, reflection, null);
                        continue;
                    }
                    if (!visible)
                    {
                        // Draw a cheap proxy only into shadow maps. Submitting the
                        // main mesh offscreen made the reported cost misleading.
                        DrawBatch(batches, kind, list, 2, ShadowCastingMode.ShadowsOnly, false, null);
                        continue;
                    }
                    var buckets = reflection ? entry.Value.Reflection : entry.Value.Main;
                    var bucket = buckets[kind];
                    if (bucket == null) buckets[kind] = bucket = new LodBucket();
                    if (bucket.Version != version)
                    {
                        PrepareBucket(entry.Value, kind, bucket, position, forward, pixelsPerUnit, orthographic);
                        bucket.Version = version;
                    }
                    var cast = kind >= 3 && !reflection ? ShadowCastingMode.On : ShadowCastingMode.Off;
                    if (bucket.Mode == 1) DrawBatch(batches, kind, list, 0, cast, reflection, null);
                    else if (bucket.Mode == 2) DrawBatch(batches, kind, list, 1, cast, reflection, null);
                    else if (bucket.Mode == 3) DrawBatch(batches, kind, list, 2, cast, reflection, null);
                    else
                    {
                        DrawBatch(batches, kind, bucket.Near, 0, cast, reflection, bucket.NearFade);
                        DrawBatch(batches, kind, bucket.Intermediate, 1, cast, reflection, bucket.IntermediateFade);
                        DrawBatch(batches, kind, bucket.Far, 2, cast, reflection, bucket.FarFade);
                    }
                }
            }
            batches.Flush();
            }
            finally
            {
                batches.Clear();
                if (reflection) reflectedSubmissionCamera = null;
                else mainSubmissionCamera = null;
            }
        }

        private Bounds CombinedTreeBounds()
        {
            var bounds = meshes[3].bounds;
            bounds.Encapsulate(meshes[5].bounds);
            bounds.Encapsulate(meshes[6].bounds);
            return bounds;
        }

        private static float ProjectedPixels(Vector3 cameraPosition, Vector3 cameraForward,
            float pixelsPerUnit, bool orthographic, Matrix4x4 matrix, Bounds bounds)
        {
            var size = Mathf.Max(bounds.size.x, Mathf.Max(bounds.size.y, bounds.size.z));
            var scale = Mathf.Max(matrix.GetColumn(0).magnitude,
                Mathf.Max(matrix.GetColumn(1).magnitude, matrix.GetColumn(2).magnitude));
            return ProjectedPixelsForCenter(cameraPosition, cameraForward, pixelsPerUnit, orthographic,
                matrix.MultiplyPoint3x4(bounds.center), size * scale);
        }

        private static float ProjectedPixelsForCenter(Vector3 cameraPosition, Vector3 cameraForward,
            float pixelsPerUnit, bool orthographic, Vector3 center, float diameter)
        {
            if (orthographic) return diameter * pixelsPerUnit;
            // Perspective projection depends on view-space depth, not radial
            // distance. The latter underestimates size near the screen edge.
            var depth = Vector3.Dot(cameraForward, center - cameraPosition);
            return diameter * pixelsPerUnit / Mathf.Max(.01f, depth);
        }

        private static int GroupLodMode(Bounds centers, float minimumDiameter, float maximumDiameter,
            Vector3 position, Vector3 forward, float pixelsPerUnit, bool orthographic)
        {
            var nearPixels = maximumDiameter * pixelsPerUnit;
            var farPixels = minimumDiameter * pixelsPerUnit;
            if (!orthographic)
            {
                var depth = Vector3.Dot(forward, centers.center - position);
                var e = centers.extents;
                var range = Mathf.Abs(forward.x) * e.x + Mathf.Abs(forward.y) * e.y + Mathf.Abs(forward.z) * e.z;
                nearPixels /= Mathf.Max(.01f, depth - range);
                farPixels /= Mathf.Max(.01f, depth + range);
            }
            if (farPixels >= DetailPixels + FadeHalfWidth) return 1;
            if (nearPixels <= ImpostorPixels - ImpostorFadeHalfWidth) return 3;
            if (farPixels >= ImpostorPixels + ImpostorFadeHalfWidth &&
                nearPixels <= DetailPixels - FadeHalfWidth) return 2;
            return 4;
        }

        private static void PrepareBucket(Chunk chunk, int kind, LodBucket bucket, Vector3 position,
            Vector3 forward, float pixelsPerUnit, bool orthographic)
        {
            bucket.Mode = GroupLodMode(chunk.LodCenters[kind], chunk.MinimumDiameter[kind],
                chunk.MaximumDiameter[kind], position, forward, pixelsPerUnit, orthographic);
            if (bucket.Mode != 4) return;
            if (bucket.Near == null)
            {
                bucket.Near = new List<Matrix4x4>(); bucket.Intermediate = new List<Matrix4x4>(); bucket.Far = new List<Matrix4x4>();
                bucket.NearFade = new float[256]; bucket.IntermediateFade = new float[256]; bucket.FarFade = new float[256];
            }
            bucket.Near.Clear(); bucket.Intermediate.Clear(); bucket.Far.Clear();
            var instances = chunk.Instances[kind];
            var lods = chunk.LodInstances[kind];
            for (var i = 0; i < instances.Count; i++)
            {
                var lod = lods[i];
                var pixels = ProjectedPixelsForCenter(position, forward, pixelsPerUnit, orthographic, lod.Center, lod.Diameter);
                var detailFade = Mathf.Clamp01((pixels - (DetailPixels - FadeHalfWidth)) / (2f * FadeHalfWidth));
                var impostorFade = Mathf.Clamp01((pixels - (ImpostorPixels - ImpostorFadeHalfWidth)) / (2f * ImpostorFadeHalfWidth));
                if (detailFade > 0f) { bucket.NearFade[bucket.Near.Count] = -detailFade; bucket.Near.Add(instances[i]); }
                if (detailFade < 1f && impostorFade > 0f)
                {
                    // Complement the full mesh in its band, or complement the
                    // impostor in the lower band. Between them the mesh is full.
                    bucket.IntermediateFade[bucket.Intermediate.Count] = detailFade > 0f ? detailFade :
                        impostorFade < 1f ? -impostorFade : 0f;
                    bucket.Intermediate.Add(instances[i]);
                }
                if (impostorFade < 1f) { bucket.FarFade[bucket.Far.Count] = impostorFade; bucket.Far.Add(instances[i]); }
            }
        }

        private void DrawBatch(BatchAccumulator batches, int kind, List<Matrix4x4> batch, int level,
            ShadowCastingMode shadows, bool reflection, float[] fades)
        {
            if (batch.Count == 0) return;
            var triangles = (level == 0 ? detailTriangleCounts[kind] : level == 1 ? intermediateTriangleCounts[kind] : distantTriangleCounts[kind]) * batch.Count;
            if (!reflection)
            {
                SubmittedTriangleCount += triangles;
                if (shadows == ShadowCastingMode.ShadowsOnly) ShadowOnlyTriangleCount += triangles;
                if (level == 0) DetailInstanceCount += batch.Count;
                else if (level == 1) IntermediateInstanceCount += batch.Count;
                else DistantInstanceCount += batch.Count;
            }
            batches.Add(kind, level, shadows, batch, fades);
        }

        private void SubmitMainBatch(BatchBuffer batch) => SubmitBatch(mainSubmissionCamera, batch, false);
        private void SubmitReflectedBatch(BatchBuffer batch) => SubmitBatch(reflectedSubmissionCamera, batch, true);

        private void SubmitBatch(Camera camera, BatchBuffer batch, bool reflection)
        {
            if (!reflection) DrawCount++;
            var kind = batch.Kind;
            var level = batch.Level;
            Graphics.DrawMeshInstanced(level == 0 ? meshes[kind] : level == 1 ? intermediateMeshes[kind] : distantMeshes[kind], 0,
                level == 2 ? distantMaterials[kind] : materials[kind],
                batch.Instances, batch.Properties(), batch.Shadows, true, 0, camera, LightProbeUsage.Off);
        }

        private static Material ImpostorMaterial(Material source, Mesh mesh, string name)
        {
            var texture = LoadTexture(name + "_lod_color");
            var normalDepth = LoadTexture(name + "_lod_normal_depth");
            var shader = Resources.Load<Shader>("ModernVegetationImpostor");
            if (shader == null) throw new InvalidOperationException("Missing Resources/ModernVegetationImpostor shader");
            var material = new Material(shader) { name = name + " relightable impostor", enableInstancing = true };
            material.SetTexture("_BaseMap", texture);
            material.SetTexture("_ImpostorData", normalDepth);
            foreach (var property in new[] { "_WindStrength", "_WindSpeed", "_WindMinY", "_WindInvHeight", "_Smoothness", "_Cutoff" })
                material.SetFloat(property, source.GetFloat(property));
            material.SetVector("_ImpostorCenter", mesh.bounds.center);
            material.SetFloat("_ImpostorDiameter", ImpostorDiameter(mesh));
            return material;
        }

        private static float ImpostorDiameter(Mesh source)
        {
            var center = source.bounds.center;
            var radiusSquared = 0f;
            foreach (var vertex in source.vertices)
                radiusSquared = Mathf.Max(radiusSquared, (vertex - center).sqrMagnitude);
            return Mathf.Sqrt(radiusSquared) * 2.02f;
        }

        // A single shader-oriented quad samples a full-sphere source atlas.
        // Bounds cover its padded rotating plane and wind in every direction.
        private static Mesh MakeImpostorCards(Mesh source)
        {
            var mesh = new Mesh { name = source.name + " multiview impostor" };
            mesh.vertices = new[] { new Vector3(-.5f, -.5f, 0), new Vector3(.5f, -.5f, 0),
                new Vector3(.5f, .5f, 0), new Vector3(-.5f, .5f, 0) };
            mesh.uv = new[] { Vector2.zero, Vector2.right, Vector2.one, Vector2.up };
            mesh.normals = new[] { Vector3.forward, Vector3.forward, Vector3.forward, Vector3.forward };
            mesh.colors = new[] { Color.white, Color.white, Color.white, Color.white };
            mesh.triangles = new[] { 0, 2, 1, 0, 3, 2 };
            var extent = ImpostorDiameter(source) * (128f / 112f);
            mesh.bounds = new Bounds(source.bounds.center, Vector3.one * (extent + 2f));
            mesh.RecalculateTangents();
            return mesh;
        }

        // Kept independent of draw submission so terrain updates and residency can
        // be checked even when Unity's test runner has no instancing-capable GPU.
        internal void RefreshChunks(Vector3 position)
        {
            if (disposed) return;
            var x = Mathf.FloorToInt(position.x / ChunkSize);
            var z = Mathf.FloorToInt(position.z / ChunkSize);
            if (dirty || residencyChanged || terrainRevision != terrain.Revision || x != centerX || z != centerZ)
            {
                if (dirty || terrainRevision != terrain.Revision) chunks.Clear();
                centerX = x;
                centerZ = z;
                UpdateChunks();
                terrainRevision = terrain.Revision;
                dirty = false;
                residencyChanged = false;
            }
        }

        private static float DistanceToChunk(float x, float z, int chunkX, int chunkZ)
        {
            var dx = Mathf.Max(0f, Mathf.Abs(x - (chunkX + .5f) * ChunkSize) - ChunkSize * .5f);
            var dz = Mathf.Max(0f, Mathf.Abs(z - (chunkZ + .5f) * ChunkSize) - ChunkSize * .5f);
            return Mathf.Sqrt(dx * dx + dz * dz);
        }

        private void UpdateChunks()
        {
            // The whole camera chunk is the residency origin, so crossing its edge
            // changes only a narrow ring. Extra margin covers every position in it.
            var originX = (centerX + .5f) * ChunkSize;
            var originZ = (centerZ + .5f) * ChunkSize;
            var extent = Mathf.CeilToInt((LandmarkRadius + ChunkSize) / ChunkSize);
            expired.Clear();
            foreach (var entry in chunks)
                if (DistanceToChunk(originX, originZ, entry.Key.x, entry.Key.y) > LandmarkRadius + ChunkSize)
                    expired.Add(entry.Key);
            foreach (var key in expired) chunks.Remove(key);
            for (var dz = -extent; dz <= extent; dz++)
            for (var dx = -extent; dx <= extent; dx++)
            {
                var key = new Vector2Int(centerX + dx, centerZ + dz);
                var distance = DistanceToChunk(originX, originZ, key.x, key.y);
                if (distance > LandmarkRadius + ChunkSize) continue;
                var detail = distance <= DetailRadius + ChunkSize;
                if (chunks.TryGetValue(key, out var chunk) && (!detail || chunk.HasDetail)) continue;
                chunk = new Chunk { HasDetail = detail };
                PopulateChunk(key.x, key.y, chunk);
                chunks[key] = chunk;
            }
        }

        private void PopulateChunk(int chunkX, int chunkZ, Chunk chunk)
        {
            for (var dz = 0; dz < CellsPerChunk; dz++)
            for (var dx = 0; dx < CellsPerChunk; dx++)
            {
                var cellX = chunkX * CellsPerChunk + dx;
                var cellZ = chunkZ * CellsPerChunk + dz;
                // Hash wrapped cells, but keep positions in the camera's current world copy.
                var seed = Hash(cellX & (WrappedCells - 1), cellZ & (WrappedCells - 1));
                var x = (cellX + 0.12f + 0.76f * Random01(seed)) * Cell;
                var z = (cellZ + 0.12f + 0.76f * Random01(seed ^ 0x9e3779b9u)) * Cell;
                if (terrain.IsWater(x, z)) continue;
                var height = terrain.SampleHeight(x, z);
                var surfaceHeight = terrain.SampleVisualHeight(x, z);
                var hx = terrain.SampleHeight(x + 1.5f, z) - terrain.SampleHeight(x - 1.5f, z);
                var hz = terrain.SampleHeight(x, z + 1.5f) - terrain.SampleHeight(x, z - 1.5f);
                var slope = Mathf.Sqrt(hx * hx + hz * hz) / 3f;
                var ground = terrain.GroundType(x, z);
                // Terrain selectors affect distribution, not an assumed mapping from IDs to biomes.
                var density = 0.78f + 0.22f * Random01((uint)ground * 7919u + 31u);
                // Riparian pockets gather ferns, shrubs and small groves while
                // keeping exposed slopes sparse. The water mask is periodic;
                // vertical terrain edits still re-ground the same placements.
                var moisture = ShoreMoisture(x, z);
                var yaw = Random01(seed ^ 0xc2b2ae35u) * 360f;
                if (chunk.HasDetail && slope < 1.15f)
                {
                    // Several independently grounded tufts form irregular patches.
                    // A broad, wrapped moisture field leaves clearings between them.
                    var cover = Mathf.Min(1, density * Mathf.Lerp(.40f, .96f, PatchDensity(x, z)) + moisture * .25f);
                    for (var tuft = 0; tuft < 4; tuft++)
                    {
                        var tuftSeed = seed ^ ((uint)(tuft + 1) * 0x27d4eb2fu);
                        if (Random01(tuftSeed) > cover) continue;
                        var px = (cellX + Random01(tuftSeed ^ 0x68bc21ebu)) * Cell;
                        var pz = (cellZ + Random01(tuftSeed ^ 0x02e5be93u)) * Cell;
                        if (terrain.IsWater(px, pz)) continue;
                        var normal = GroundNormal(px, pz);
                        if (normal.y < .65f) continue;
                        var choice = Random01(tuftSeed ^ 0x85ebca6bu);
                        var kind = choice < Mathf.Lerp(.68f,.45f,moisture) ? 0 : choice < Mathf.Lerp(.88f,.82f,moisture) ? 1 : 2;
                        var scale = .85f + .75f * Random01(tuftSeed ^ 0x165667b1u);
                        // Large fronds over a submerged bank read as floating
                        // plants. Their roots need a dry supporting footprint.
                        var support=(kind==0?.20f:kind==1?.65f:.45f)*scale;
                        if(!DryFootprint(px,pz,support)) continue;
                        Add(chunk, kind, px, terrain.SampleVisualHeight(px, pz) - .05f, pz,
                            Random01(tuftSeed ^ 0xc2b2ae35u) * 360f, scale, normal);
                    }
                }
                var landmark = Random01(seed ^ 0x165667b1u);
                if (landmark < .045f * Mathf.Lerp(.5f, 1.3f, PatchDensity(x, z)) + moisture * .055f && slope < .55f && DryFootprint(x,z,.7f))
                {
                    // Scanned trees follow the same terrain as ground cover, independently of actors.
                    var scale = .65f + .95f * Random01(seed ^ 0xd3a2646cu);
                    Add(chunk, 3, x, surfaceHeight - .08f, z, yaw, scale, Vector3.up);
                }
                else if (landmark > Mathf.Lerp(.88f,.97f,Mathf.Clamp01((height-15)/35)) && slope < 1.15f)
                {
                    var scale = .45f + 1.40f * Random01(seed ^ 0xfd7046c5u);
                    var normal = GroundNormal(x, z);
                    // Scanned boulders have rounded undersides. Half-burial makes
                    // them supported outcrops rather than rocks perched on one
                    // point of a curved slope.
                    var anchor = new Vector3(x, surfaceHeight, z) - normal * (meshes[4].bounds.size.y*.48f*scale);
                    Add(chunk, 4, anchor.x, anchor.y, anchor.z, yaw, scale, normal);
                    // Talus gathers around boulders on supported ground. Sparse
                    // isolated boulders on near-vertical faces looked suspended.
                    if (chunk.HasDetail)
                    for (var fragment = 0; fragment < 3; fragment++)
                    {
                        var fragmentSeed = seed ^ ((uint)(fragment + 1) * 0x92d78269u);
                        var angle = Random01(fragmentSeed) * Mathf.PI * 2f;
                        var radius = scale * (1.1f + Random01(fragmentSeed ^ 0x68bc21ebu));
                        var px = x + Mathf.Cos(angle) * radius;
                        var pz = z + Mathf.Sin(angle) * radius;
                        if (terrain.IsWater(px, pz)) continue;
                        var fragmentNormal = GroundNormal(px, pz);
                        if (fragmentNormal.y < .70f) continue;
                        var fragmentScale = .12f + .28f * Random01(fragmentSeed ^ 0x02e5be93u);
                        var fragmentAnchor = new Vector3(px, terrain.SampleVisualHeight(px, pz), pz)
                            - fragmentNormal * (.25f * fragmentScale);
                        Add(chunk, 4, fragmentAnchor.x, fragmentAnchor.y, fragmentAnchor.z,
                            Random01(fragmentSeed ^ 0xc2b2ae35u) * 360f, fragmentScale, fragmentNormal);
                    }
                }
            }
        }

        private Vector3 GroundNormal(float x, float z)
        {
            const float offset = .5f;
            return new Vector3(terrain.SampleHeight(x - offset, z) - terrain.SampleHeight(x + offset, z),
                2 * offset, terrain.SampleHeight(x, z - offset) - terrain.SampleHeight(x, z + offset)).normalized;
        }

        private bool DryFootprint(float x,float z,float radius) =>
            !terrain.IsWater(x-radius,z) && !terrain.IsWater(x+radius,z)
            && !terrain.IsWater(x,z-radius) && !terrain.IsWater(x,z+radius);

        private float ShoreMoisture(float x, float z)
        {
            var wet = 0f;
            for (var i=0;i<4;i++)
            {
                var dx=i==0?1:i==1?-1:0;var dz=i==2?1:i==3?-1:0;
                if (terrain.IsWater(x+dx*12,z+dz*12)) wet+=.35f;
                if (terrain.IsWater(x+dx*24,z+dz*24)) wet+=.15f;
            }
            return Mathf.Clamp01(wet);
        }

        private static float PatchDensity(float x, float z)
        {
            const int cells = 64; // 16-metre patches repeat with the 1024-metre world.
            var px = x / 16f;
            var pz = z / 16f;
            var ix = Mathf.FloorToInt(px);
            var iz = Mathf.FloorToInt(pz);
            var fx = Mathf.SmoothStep(0, 1, px - ix);
            var fz = Mathf.SmoothStep(0, 1, pz - iz);
            return Mathf.Lerp(Mathf.Lerp(Random01(Hash(ix & (cells - 1), iz & (cells - 1))),
                Random01(Hash((ix + 1) & (cells - 1), iz & (cells - 1))), fx),
                Mathf.Lerp(Random01(Hash(ix & (cells - 1), (iz + 1) & (cells - 1))),
                Random01(Hash((ix + 1) & (cells - 1), (iz + 1) & (cells - 1))), fx), fz);
        }

        private void Add(Chunk chunk, int kind, float x, float y, float z, float yaw, float scale, Vector3 normal)
        {
            if(ExcludedFootprint?.Invoke(x,z)==true) return;
            var matrix = Matrix4x4.TRS(new Vector3(x, y, z),
                Quaternion.FromToRotation(Vector3.up, normal) * Quaternion.Euler(0f, yaw, 0f), Vector3.one * scale);
            chunk.Instances[kind].Add(matrix);
            IncludeBounds(chunk, meshes[kind].bounds, matrix);
            IncludeLod(chunk, kind, matrix, kind == 3 ? treeLocalBounds : meshes[kind].bounds, scale);
            if (kind == 3)
            {
                chunk.Instances[5].Add(matrix);
                chunk.Instances[6].Add(matrix);
                IncludeBounds(chunk, meshes[5].bounds, matrix);
                IncludeBounds(chunk, meshes[6].bounds, matrix);
                IncludeLod(chunk, 5, matrix, treeLocalBounds, scale);
                IncludeLod(chunk, 6, matrix, treeLocalBounds, scale);
            }
        }

        private static void IncludeLod(Chunk chunk, int kind, Matrix4x4 matrix, Bounds local, float scale)
        {
            var center = matrix.MultiplyPoint3x4(local.center);
            var diameter = Mathf.Max(local.size.x, Mathf.Max(local.size.y, local.size.z)) * scale;
            if (chunk.LodInstances[kind].Count == 0) chunk.LodCenters[kind] = new Bounds(center, Vector3.zero);
            else chunk.LodCenters[kind].Encapsulate(center);
            chunk.MinimumDiameter[kind] = Mathf.Min(chunk.MinimumDiameter[kind], diameter);
            chunk.MaximumDiameter[kind] = Mathf.Max(chunk.MaximumDiameter[kind], diameter);
            chunk.LodInstances[kind].Add(new LodInstance { Center = center, Diameter = diameter });
        }

        private static void IncludeBounds(Chunk chunk, Bounds local, Matrix4x4 matrix)
        {
            var center = matrix.MultiplyPoint3x4(local.center);
            var e = local.extents;
            var worldExtents = new Vector3(
                Mathf.Abs(matrix.m00) * e.x + Mathf.Abs(matrix.m01) * e.y + Mathf.Abs(matrix.m02) * e.z,
                Mathf.Abs(matrix.m10) * e.x + Mathf.Abs(matrix.m11) * e.y + Mathf.Abs(matrix.m12) * e.z,
                Mathf.Abs(matrix.m20) * e.x + Mathf.Abs(matrix.m21) * e.y + Mathf.Abs(matrix.m22) * e.z);
            // Allow for vertex-shader wind beyond the imported static bounds.
            var bounds = new Bounds(center, worldExtents * 2f + Vector3.one * 2f);
            if (chunk.HasBounds) chunk.Bounds.Encapsulate(bounds);
            else { chunk.Bounds = bounds; chunk.HasBounds = true; }
        }

        private static uint Hash(int x, int z)
        {
            unchecked
            {
                uint h = (uint)x * 0x8da6b343u ^ (uint)z * 0xd8163841u ^ 0xcb1ab31fu;
                h ^= h >> 16; h *= 0x7feb352du; h ^= h >> 15; h *= 0x846ca68bu; h ^= h >> 16;
                return h;
            }
        }

        private static float Random01(uint value)
        {
            unchecked
            {
                value ^= value >> 16; value *= 0x7feb352du; value ^= value >> 15;
                value *= 0x846ca68bu; value ^= value >> 16;
                return (value & 0x00ffffffu) / 16777216f;
            }
        }

        private static Material ScannedMaterial(Shader shader, string asset, Mesh mesh, bool foliage)
        {
            var result = new Material(shader) { name = "Poly Haven " + asset, enableInstancing = true };
            try
            {
            result.SetFloat("_UseTextures", 1f);
            result.SetTexture("_BaseMap", LoadTexture(asset + "_diff_1k"));
            result.SetTexture("_NormalMap", LoadTexture(asset + "_nor_gl_1k"));
            result.SetTexture("_RoughnessMap", LoadTexture(asset + "_rough_1k"));
            result.SetFloat("_UseNormalMap", 1f);
            result.SetFloat("_UseRoughnessMap", 1f);
            result.SetFloat("_WindUseHeight", 1f);
            result.SetFloat("_WindMinY", mesh.bounds.min.y);
            result.SetFloat("_WindInvHeight", 1f / Mathf.Max(mesh.bounds.size.y, 0.001f));
            result.SetFloat("_WindStrength", foliage ? 0.12f : 0f);
            if (foliage)
            {
                result.SetTexture("_AlphaMap", LoadTexture(asset + "_alpha_1k"));
                result.SetFloat("_UseAlphaMap", 1f);
            }
            return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        private static Texture2D LoadTexture(string name)
        {
            var texture = Resources.Load<Texture2D>("ModernNature/" + name);
            if (texture == null) throw new InvalidOperationException("Missing scanned nature texture: " + name);
            return texture;
        }

#if UNITY_EDITOR
        // Cluster nearby vertices only when their texture coordinates also agree.
        // Keeping the original vertex attributes preserves the scanned materials,
        // normals and alpha cutouts; degenerate triangles disappear at distance.
        private static Mesh MakeDistantMesh(Mesh source, float cellFraction)
        {
            var result = UnityEngine.Object.Instantiate(source);
            try
            {
                result.name = source.name + " distant";
                var vertices = source.vertices;
                var uv = source.uv;
                var sourceTriangles = new int[source.subMeshCount][];
                var originalCount = 0;
                for (var sub = 0; sub < sourceTriangles.Length; sub++)
                {
                    sourceTriangles[sub] = source.GetTriangles(sub);
                    originalCount += sourceTriangles[sub].Length;
                }
                var width = Mathf.Max(source.bounds.size.x, Mathf.Max(source.bounds.size.y, source.bounds.size.z));
                var step = Mathf.Max(width * cellFraction, .0001f);
                var representatives = new Dictionary<(int, int, int, int, int), int>();
                var indices = new int[vertices.Length];
                for (var i = 0; i < vertices.Length; i++)
                {
                    var p = vertices[i] - source.bounds.min;
                    var tex = uv.Length == vertices.Length ? uv[i] : Vector2.zero;
                    var key = (Mathf.RoundToInt(p.x / step), Mathf.RoundToInt(p.y / step),
                        Mathf.RoundToInt(p.z / step), Mathf.RoundToInt(tex.x * 32f), Mathf.RoundToInt(tex.y * 32f));
                    if (!representatives.TryGetValue(key, out var representative))
                    {
                        representative = i;
                        representatives.Add(key, i);
                    }
                    indices[i] = representative;
                }
                var reduced = new int[sourceTriangles.Length][];
                var reducedCount = 0;
                for (var sub = 0; sub < sourceTriangles.Length; sub++)
                {
                    var triangles = sourceTriangles[sub];
                    var kept = new List<int>(triangles.Length);
                    for (var i = 0; i < triangles.Length; i += 3)
                    {
                        var a = indices[triangles[i]];
                        var b = indices[triangles[i + 1]];
                        var c = indices[triangles[i + 2]];
                        if (a == b || b == c || c == a) continue;
                        kept.Add(a); kept.Add(b); kept.Add(c);
                    }
                    reduced[sub] = kept.ToArray();
                    reducedCount += kept.Count;
                }
                // Never discard a mesh if a pathological input collapses its silhouette.
                var candidateCount = reducedCount;
                if (reducedCount >= originalCount / 3)
                    for (var sub = 0; sub < reduced.Length; sub++) result.SetTriangles(reduced[sub], sub, false);
                else reducedCount = originalCount;
                Debug.Log($"MC_MODERN_NATURE_LOD name={source.name} triangles={originalCount / 3}->{reducedCount / 3} candidate={candidateCount / 3}");
                return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        // The scanned crown is thousands of disconnected, two-triangle alpha
        // cards. Collapse would destroy their UV silhouettes, so pair their shared
        // edges and retain whole cards across spatial cells and three orientations.
        private static Mesh MakeDistantLeafCards(Mesh source)
        {
            var result = UnityEngine.Object.Instantiate(source);
            try
            {
                result.name = source.name + " distant";
                var vertices = source.vertices;
                var positions = new Dictionary<Vector3, int>();
                var welded = new int[vertices.Length];
                for (var i = 0; i < vertices.Length; i++)
                {
                    if (!positions.TryGetValue(vertices[i], out var id))
                    {
                        id = positions.Count;
                        positions.Add(vertices[i], id);
                    }
                    welded[i] = id;
                }
                var width = Mathf.Max(source.bounds.size.x, Mathf.Max(source.bounds.size.y, source.bounds.size.z));
                var step = Mathf.Max(width * .035f, .0001f);
                var originalCount = 0;
                var reducedCount = 0;
                var pairedCount = 0;
                for (var sub = 0; sub < source.subMeshCount; sub++)
                {
                    var triangles = source.GetTriangles(sub);
                    var count = triangles.Length / 3;
                    originalCount += count;
                    var pairs = new int[count];
                    for (var i = 0; i < count; i++) pairs[i] = -1;
                    var edges = new Dictionary<(int, int), int>();
                    for (var tri = 0; tri < count; tri++)
                    {
                        for (var edge = 0; edge < 3 && pairs[tri] < 0; edge++)
                        {
                            var a = welded[triangles[tri * 3 + edge]];
                            var b = welded[triangles[tri * 3 + (edge + 1) % 3]];
                            if (a == b) continue;
                            var key = a < b ? (a, b) : (b, a);
                            if (edges.TryGetValue(key, out var other) && pairs[other] < 0)
                            {
                                var n = TriangleNormal(vertices, triangles, tri);
                                var m = TriangleNormal(vertices, triangles, other);
                                if (Vector3.Dot(n, m) > .995f)
                                {
                                    pairs[tri] = other;
                                    pairs[other] = tri;
                                    pairedCount += 2;
                                }
                            }
                            else if (!edges.ContainsKey(key)) edges.Add(key, tri);
                        }
                    }
                    var winners = new Dictionary<(int, int, int, int), (int triangle, float score)>();
                    var extremes = new int[6];
                    var extremeValues = new float[6];
                    for (var i = 0; i < extremes.Length; i++) { extremes[i] = -1; extremeValues[i] = float.NegativeInfinity; }
                    for (var tri = 0; tri < count; tri++)
                    {
                        var other = pairs[tri];
                        if (other < 0 || other < tri) continue;
                        var center = Vector3.zero;
                        for (var v = 0; v < 3; v++)
                        {
                            var a = vertices[triangles[tri * 3 + v]];
                            var b = vertices[triangles[other * 3 + v]];
                            center += a + b;
                            IncludeCardExtremes(a, tri, extremes, extremeValues);
                            IncludeCardExtremes(b, tri, extremes, extremeValues);
                        }
                        center /= 6f;
                        var n = TriangleNormal(vertices, triangles, tri);
                        var ax = Mathf.Abs(n.x); var ay = Mathf.Abs(n.y); var az = Mathf.Abs(n.z);
                        var axis = ax >= ay && ax >= az ? 0 : ay >= az ? 1 : 2;
                        var relative = center - source.bounds.min;
                        var cell = (Mathf.FloorToInt(relative.x / step), Mathf.FloorToInt(relative.y / step),
                            Mathf.FloorToInt(relative.z / step), axis);
                        var score = (center - source.bounds.center).sqrMagnitude;
                        if (!winners.TryGetValue(cell, out var winner) || score > winner.score)
                            winners[cell] = (tri, score);
                    }
                    var selected = new HashSet<int>();
                    foreach (var winner in winners.Values) selected.Add(winner.triangle);
                    foreach (var extreme in extremes) if (extreme >= 0) selected.Add(extreme);
                    var kept = new List<int>(triangles.Length);
                    for (var tri = 0; tri < count; tri++)
                    {
                        var other = pairs[tri];
                        if (other >= 0 && (other < tri || !selected.Contains(tri))) continue;
                        for (var v = 0; v < 3; v++) kept.Add(triangles[tri * 3 + v]);
                        if (other >= 0)
                            for (var v = 0; v < 3; v++) kept.Add(triangles[other * 3 + v]);
                    }
                    reducedCount += kept.Count / 3;
                    result.SetTriangles(kept, sub, false);
                }
                Debug.Log($"MC_MODERN_NATURE_LOD name={source.name} triangles={originalCount}->{reducedCount} paired={pairedCount}");
                return result;
            }
            catch
            {
                Destroy(result);
                throw;
            }
        }

        private static void IncludeCardExtremes(Vector3 point, int triangle, int[] extremes, float[] values)
        {
            for (var axis = 0; axis < 3; axis++)
            for (var side = 0; side < 2; side++)
            {
                var index = axis * 2 + side;
                var value = point[axis] * (side == 0 ? -1f : 1f);
                if (value > values[index]) { values[index] = value; extremes[index] = triangle; }
            }
        }

        private static Vector3 TriangleNormal(Vector3[] vertices, int[] triangles, int triangle)
        {
            var a = vertices[triangles[triangle * 3]];
            var b = vertices[triangles[triangle * 3 + 1]];
            var c = vertices[triangles[triangle * 3 + 2]];
            return Vector3.Cross(b - a, c - a).normalized;
        }

#endif

        internal static string IntermediateResourceName(int kind)
        {
            if (kind < 0 || kind >= IntermediateResourceNames.Length) throw new ArgumentOutOfRangeException(nameof(kind));
            return IntermediateResourceNames[kind];
        }

        private static Mesh LoadSourceMesh(int kind, Bounds treeBounds)
        {
            switch (kind)
            {
                case 0: return LoadScannedMesh("grass_medium_01", "grass_medium_01_mid_b_LOD0", 1.4f);
                case 1: return LoadScannedMesh("fern_02", "fern_02_b", 1.7f);
                case 2: return LoadScannedMesh("shrub_02", "shrub_02_b_LOD0", 1.7f);
                case 3: return LoadScannedMesh("tree_small_02_derived", "tree_trunk", 4.5f, treeBounds);
                case 4: return LoadScannedMesh("rock_moss_set_01", "rock_moss_set_01_rock03", 2.4f);
                case 5: return LoadScannedMesh("tree_small_02_derived", "tree_branches", 4.5f, treeBounds);
                case 6: return LoadScannedMesh("tree_small_02_derived", "tree_leaves", 4.5f, treeBounds);
                default: throw new ArgumentOutOfRangeException(nameof(kind));
            }
        }

#if UNITY_EDITOR
        // Authoring-only factory. The player loads its prebuilt Mesh assets.
        internal static Mesh CreateIntermediateForAuthoring(int kind)
        {
            var source = LoadSourceMesh(kind, ScannedTreeBounds());
            try { return kind == 6 ? MakeDistantLeafCards(source) : MakeDistantMesh(source, kind == 5 ? .01f : .045f); }
            finally { Destroy(source); }
        }
#endif

        // FBX collections contain separate objects with display offsets. Bake the
        // selected object's import transform, then center its base on the terrain.
        private static Mesh LoadScannedMesh(string asset, string part, float targetWidth, Bounds? sharedBounds = null)
        {
            var resourceName = asset.EndsWith("_derived", StringComparison.Ordinal) || asset.EndsWith("_lod", StringComparison.Ordinal) ? asset : asset + "_1k";
            var prefab = Resources.Load<GameObject>("ModernNature/" + resourceName);
            if (prefab == null) throw new InvalidOperationException("Missing scanned nature model: " + asset);
            MeshFilter selected = null;
            var filters = prefab.GetComponentsInChildren<MeshFilter>(true);
            foreach (var filter in filters)
            {
                if (filter.sharedMesh == null || (filter.name != part && filter.sharedMesh.name != part)) continue;
                if (selected != null) throw new InvalidOperationException("Ambiguous scanned nature mesh: " + part);
                selected = filter;
            }
            if (selected == null || selected.sharedMesh == null)
                throw new InvalidOperationException("Missing scanned nature mesh: " + part);
            var source = selected.sharedMesh;
            var mesh = UnityEngine.Object.Instantiate(source);
            try
            {
            mesh.name = "Scanned " + part;
            var transform = selected.transform.localToWorldMatrix;
            var normalTransform = transform.inverse.transpose;
            var vertices = mesh.vertices;
            var normals = mesh.normals;
            var tangents = mesh.tangents;
            for (var i = 0; i < vertices.Length; i++)
            {
                vertices[i] = transform.MultiplyPoint3x4(vertices[i]);
                if (normals.Length == vertices.Length) normals[i] = normalTransform.MultiplyVector(normals[i]).normalized;
                if (tangents.Length == vertices.Length)
                {
                    var t = transform.MultiplyVector(new Vector3(tangents[i].x, tangents[i].y, tangents[i].z)).normalized;
                    tangents[i] = new Vector4(t.x, t.y, t.z, tangents[i].w * (transform.determinant < 0f ? -1f : 1f));
                }
            }
            if (transform.determinant < 0f)
            {
                for (var sub = 0; sub < mesh.subMeshCount; sub++)
                {
                    var triangles = mesh.GetTriangles(sub);
                    for (var i = 0; i < triangles.Length; i += 3)
                    { var swap = triangles[i]; triangles[i] = triangles[i + 1]; triangles[i + 1] = swap; }
                    mesh.SetTriangles(triangles, sub);
                }
            }
            mesh.vertices = vertices;
            mesh.RecalculateBounds();
            var bounds = sharedBounds ?? mesh.bounds;
            var width = Mathf.Max(bounds.size.x, bounds.size.z);
            if (width < 0.0001f) throw new InvalidOperationException("Empty scanned nature mesh: " + part);
            var scale = targetWidth / width;
            var center = new Vector3(bounds.center.x, bounds.min.y, bounds.center.z);
            var colors = new Color[vertices.Length];
            for (var i = 0; i < vertices.Length; i++) { vertices[i] = (vertices[i] - center) * scale; colors[i] = Color.white; }
            mesh.vertices = vertices;
            mesh.colors = colors;
            if (normals.Length == vertices.Length) mesh.normals = normals; else mesh.RecalculateNormals();
            if (tangents.Length == vertices.Length) mesh.tangents = tangents; else mesh.RecalculateTangents();
            mesh.RecalculateBounds();
            ulong triangleCount = 0;
            for (var sub = 0; sub < mesh.subMeshCount; sub++) triangleCount += mesh.GetIndexCount(sub) / 3;
            Debug.Log($"MC_MODERN_NATURE_MESH name={part} vertices={mesh.vertexCount} triangles={triangleCount}");
            return mesh;
            }
            catch
            {
                Destroy(mesh);
                throw;
            }
        }

        // All three scanned tree parts must share one origin and scale.
        private static Bounds ScannedTreeBounds()
        {
            var prefab = Resources.Load<GameObject>("ModernNature/tree_small_02_derived");
            if (prefab == null) throw new InvalidOperationException("Missing scanned nature model: tree_small_02_derived");
            var found = false;
            var bounds = new Bounds();
            foreach (var filter in prefab.GetComponentsInChildren<MeshFilter>(true))
            {
                if (filter.sharedMesh == null || (filter.name != "tree_trunk" &&
                    filter.name != "tree_branches" && filter.name != "tree_leaves")) continue;
                var part = filter.sharedMesh.bounds;
                var matrix = filter.transform.localToWorldMatrix;
                for (var x = -1; x <= 1; x += 2)
                for (var y = -1; y <= 1; y += 2)
                for (var z = -1; z <= 1; z += 2)
                {
                    var point = matrix.MultiplyPoint3x4(part.center + Vector3.Scale(part.extents, new Vector3(x, y, z)));
                    if (!found) { bounds = new Bounds(point, Vector3.zero); found = true; }
                    else bounds.Encapsulate(point);
                }
            }
            if (!found) throw new InvalidOperationException("Missing scanned tree meshes");
            return bounds;
        }

        private sealed class Geometry
        {
            private readonly List<Vector3> vertices = new List<Vector3>();
            private readonly List<Color> colors = new List<Color>();
            private readonly List<Vector2> uv = new List<Vector2>();
            private readonly List<int> triangles = new List<int>();

            internal void Triangle(Vector3 a, Vector3 b, Vector3 c, Color color, float wa, float wb, float wc)
            {
                var index = vertices.Count;
                vertices.Add(a); vertices.Add(b); vertices.Add(c);
                colors.Add(color); colors.Add(color); colors.Add(color);
                uv.Add(new Vector2(0, wa)); uv.Add(new Vector2(0, wb)); uv.Add(new Vector2(0, wc));
                triangles.Add(index); triangles.Add(index + 1); triangles.Add(index + 2);
            }

            internal Mesh Finish(string name)
            {
                var mesh = new Mesh { name = name };
                try
                {
                    mesh.SetVertices(vertices); mesh.SetColors(colors); mesh.SetUVs(0, uv); mesh.SetTriangles(triangles, 0);
                    mesh.RecalculateNormals(); mesh.RecalculateBounds();
                    return mesh;
                }
                catch
                {
                    Destroy(mesh);
                    throw;
                }
            }
        }

        private static Mesh MakeGrass()
        {
            var g = new Geometry();
            var palette = new[] { new Color(0.24f, 0.38f, 0.29f), new Color(0.38f, 0.44f, 0.30f), new Color(0.30f, 0.42f, 0.37f) };
            for (var i = 0; i < 17; i++)
            {
                var a = i * 2.39996f;
                var r = 0.12f + 0.50f * Random01((uint)i + 17u);
                var root = new Vector3(Mathf.Cos(a) * r, 0, Mathf.Sin(a) * r);
                var side = new Vector3(-Mathf.Sin(a), 0, Mathf.Cos(a)) * (0.055f + 0.045f * Random01((uint)i + 91u));
                var tip = root + new Vector3(Mathf.Cos(a) * 0.30f + 0.20f, 0.65f + 0.65f * Random01((uint)i + 81u), Mathf.Sin(a) * 0.30f);
                g.Triangle(root - side, tip, root + side, palette[i % palette.Length], 0, 1, 0);
            }
            return g.Finish("Moor grass clump");
        }

        private static Mesh MakeFern()
        {
            var g = new Geometry();
            var green = new Color(0.25f, 0.39f, 0.31f);
            for (var n = 0; n < 6; n++)
            {
                var angle = n * Mathf.PI / 3f;
                var dir = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
                var side = new Vector3(-dir.z, 0, dir.x);
                var root = new Vector3(0, 0.06f, 0);
                var tip = dir * 0.86f + Vector3.up * 0.75f;
                g.Triangle(root - side * 0.025f, tip, root + side * 0.025f, green * 0.75f, 0, 1, 0);
                for (var j = 1; j <= 4; j++)
                {
                    var t = j / 5f;
                    var center = Vector3.Lerp(root, tip, t) + Vector3.up * (0.18f * Mathf.Sin(t * Mathf.PI));
                    var next = Vector3.Lerp(root, tip, t + 0.17f);
                    var width = 0.28f * (1f - t * 0.7f);
                    g.Triangle(center, next, center + side * width + dir * 0.09f, green, t, t + 0.17f, t);
                    g.Triangle(center, center - side * width + dir * 0.09f, next, green * 1.12f, t, t, t + 0.17f);
                }
            }
            return g.Finish("Wind fern");
        }

        private static Mesh MakeShrub()
        {
            var g = new Geometry();
            var colors = new[] { new Color(0.29f, 0.36f, 0.30f), new Color(0.39f, 0.41f, 0.32f), new Color(0.33f, 0.35f, 0.40f) };
            for (var i = 0; i < 11; i++)
            {
                var a = i * 2.39996f;
                var dir = new Vector3(Mathf.Cos(a), 0, Mathf.Sin(a));
                var side = new Vector3(-dir.z, 0, dir.x);
                var basePos = dir * 0.12f;
                var tip = dir * (0.5f + 0.18f * Random01((uint)i + 10u)) + Vector3.up * (0.65f + 0.45f * Random01((uint)i + 30u));
                var mid = Vector3.Lerp(basePos, tip, 0.55f);
                g.Triangle(basePos, tip, mid + side * 0.32f, colors[i % 3], 0, 1, 0.55f);
                g.Triangle(basePos, mid - side * 0.32f, tip, colors[(i + 1) % 3], 0, 0.55f, 1);
            }
            return g.Finish("Moor shrub");
        }

        private static Mesh MakeTree()
        {
            var g = new Geometry();
            var bark = new Color(0.25f, 0.23f, 0.22f);
            const int sides = 7;
            for (var i = 0; i < sides; i++)
            {
                var a = i * Mathf.PI * 2f / sides;
                var b = (i + 1) * Mathf.PI * 2f / sides;
                var p = new Vector3(Mathf.Cos(a) * 0.24f, 0, Mathf.Sin(a) * 0.24f);
                var q = new Vector3(Mathf.Cos(b) * 0.24f, 0, Mathf.Sin(b) * 0.24f);
                var top = new Vector3(0.35f, 4.4f, 0);
                g.Triangle(p, top, q, bark * (0.85f + 0.05f * i), 0, 0.25f, 0);
            }
            var needles = new[] { new Color(0.16f, 0.29f, 0.27f), new Color(0.21f, 0.33f, 0.29f), new Color(0.27f, 0.37f, 0.33f) };
            for (var tier = 0; tier < 4; tier++)
            {
                var y = 1.05f + tier * 0.78f;
                var radius = 1.35f - tier * 0.23f;
                var apex = new Vector3(0.18f + tier * 0.07f, y + 1.40f, 0);
                for (var i = 0; i < sides; i++)
                {
                    var a = (i + tier * 0.23f) * Mathf.PI * 2f / sides;
                    var b = (i + 1 + tier * 0.23f) * Mathf.PI * 2f / sides;
                    var p = new Vector3(Mathf.Cos(a) * radius, y + (i % 2) * 0.12f, Mathf.Sin(a) * radius);
                    var q = new Vector3(Mathf.Cos(b) * radius, y + ((i + 1) % 2) * 0.12f, Mathf.Sin(b) * radius);
                    g.Triangle(p, apex, q, needles[(tier + i) % 3], y / 5f, 1, y / 5f);
                    g.Triangle(q, new Vector3(0.18f, y + 0.24f, 0), p, needles[0] * 0.75f, y / 5f, y / 5f, y / 5f);
                }
            }
            // Uneven side boughs break up the regular layered silhouette.
            for (var branch = 0; branch < 3; branch++)
            {
                var angle = branch * 2.4f + 0.4f;
                var dir = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
                var side = new Vector3(-dir.z, 0, dir.x);
                var root = Vector3.up * (1.25f + branch * 0.68f);
                var tip = root + dir * (1.48f - branch * 0.12f) + Vector3.up * 0.38f;
                var fork = tip - dir * 0.37f;
                g.Triangle(root - side * 0.07f, tip, root + side * 0.07f, bark, 0.2f, 0.55f, 0.2f);
                g.Triangle(fork - side * 0.43f, tip + Vector3.up * 0.48f, fork + side * 0.43f,
                    needles[(branch + 1) % 3], 0.45f, 0.7f, 0.45f);
                g.Triangle(fork - dir * 0.25f, tip + Vector3.up * 0.48f, tip + dir * 0.30f,
                    needles[branch % 3], 0.45f, 0.7f, 0.55f);
            }
            return g.Finish("Decorative windswept conifer");
        }

        private static Mesh MakeRock()
        {
            var g = new Geometry();
            var colors = new[] { new Color(0.34f, 0.36f, 0.39f), new Color(0.40f, 0.41f, 0.42f), new Color(0.29f, 0.32f, 0.34f) };
            const int sides = 7;
            var top = new Vector3(0.16f, 0.95f, -0.08f);
            for (var i = 0; i < sides; i++)
            {
                var a = i * Mathf.PI * 2f / sides;
                var b = (i + 1) * Mathf.PI * 2f / sides;
                var r = 0.85f + 0.28f * Random01((uint)i + 37u);
                var s = 0.85f + 0.28f * Random01((uint)((i + 1) % sides) + 37u);
                var p = new Vector3(Mathf.Cos(a) * r, 0, Mathf.Sin(a) * r * 0.75f);
                var q = new Vector3(Mathf.Cos(b) * s, 0, Mathf.Sin(b) * s * 0.75f);
                var shoulder = (p + q) * 0.31f + Vector3.up * (0.55f + 0.2f * Random01((uint)i + 11u));
                g.Triangle(p, shoulder, q, colors[i % 3], 0, 0, 0);
                g.Triangle(p, top, shoulder, colors[(i + 1) % 3], 0, 0, 0);
                g.Triangle(shoulder, top, q, colors[(i + 2) % 3], 0, 0, 0);
            }
            return g.Finish("Moor rock outcrop");
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            foreach (var mesh in meshes) Destroy(mesh);
            foreach (var mesh in intermediateMeshes) Destroy(mesh);
            foreach (var mesh in distantMeshes) Destroy(mesh);
            foreach (var value in materials) Destroy(value);
            foreach (var value in distantMaterials) Destroy(value);
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
