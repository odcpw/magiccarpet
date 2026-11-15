using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // A presentation of the live outdoor heightfield. Simulation/collision remains in Rust.
    internal sealed class ModernTerrainMesh : IDisposable
    {
        private const int MapCells = 256;
        private const int SectionCells = 32;
        private const int Sections = MapCells / SectionCells;
        private const int Subdivisions = 2;
        private const int DetailCells = SectionCells * Subdivisions;
        private const int Side = DetailCells + 1;
        private const float CellSize = 4f;
        private const float DetailSize = CellSize / Subdivisions;
        private const float Period = MapCells * CellSize;
        private const float SectionSize = SectionCells * CellSize;
        private const int PlaneSize = MapCells * MapCells;
        private const float DetailEnterDistance = 192f;
        private const float DetailExitDistance = 224f;

        private sealed class Copy
        {
            internal GameObject Object;
            internal Transform Transform;
            internal MeshFilter Filter;
            internal MeshRenderer Renderer;
            internal MeshRenderer ShoreRenderer;
            internal bool Detail;
        }

        private sealed class Section
        {
            internal Mesh Mesh;
            internal Mesh FarMesh;
            internal Mesh ShoreMesh;
            internal Mesh VoxelMesh;
            internal Mesh VoxelFarMesh;
            internal bool VoxelDirty = true;
            internal bool VoxelFarDirty = true;
            internal bool HasWater;
            internal Bounds WaterBounds;
            internal int DetailTriangles;
            internal int FarTriangles;
            internal readonly Copy[] Copies = new Copy[9];
        }

        private readonly Transform root;
        private readonly bool voxel;
        private readonly Material[] materials;
        private readonly ModernShoreMask sourceShore;
        private readonly Section[] sections = new Section[Sections * Sections];
        private readonly byte[] heights = new byte[PlaneSize];
        private readonly byte[] types = new byte[PlaneSize];
        private readonly byte[] angles = new byte[PlaneSize];
        private readonly float[] landform = new float[PlaneSize];
        private readonly bool[] landformDirty = new bool[PlaneSize];
        private readonly float[] detailHeights = new float[512 * 512];
        private readonly bool[] detailHeightValid = new bool[512 * 512];
        private readonly bool[] dirty = new bool[Sections * Sections];
        private readonly ulong[] waterBedRevisions = new ulong[Sections * Sections];
        private readonly ulong[] waterBedVertexRevisions = new ulong[512 * 512];
        private readonly Plane[] frustumPlanes = new Plane[6];
        private static readonly int WaveStrength = Shader.PropertyToID("_WaveStrength");
        private bool initialized;
        private bool disposed;
        private bool presented;
        private int presentedWorldX, presentedWorldZ;
        private ulong lastGeneration;
        internal ulong Revision { get; private set; }
        internal ulong WaterBedResetRevision { get; private set; }
        internal int VisibleTriangleCount { get; private set; }
        internal int VisibleDetailCopies { get; private set; }
        internal int VisibleFarCopies { get; private set; }
        // Values up to one world period are supported by the 3x3 copy window.
        internal float ViewDistance { get; set; } = 320f;
        // Set before the first Update; local voxel physics replaces this aligned patch.
        internal Bounds? VoxelPatch { get; set; }
        internal Func<float,float,float?> VoxelHeightSampler { get; private set; }
        private Bounds? voxelHeightFootprint;
        internal ulong WaterBedVersion { get; private set; }

        internal ulong WaterBedRevision(int x, int z) => waterBedRevisions[x + z * Sections];
        internal ulong WaterBedVertexRevision(int x, int z) => waterBedVertexRevisions[(x & 511) + (z & 511) * 512];

        // Sampler coverage is expressed in the same absolute world coordinates as
        // SampleHeight. Invalidate both footprints when replacing or removing it.
        internal void SetVoxelHeightSampler(Func<float,float,float?> sampler, Bounds? footprint)
        {
            if (sampler != null && !footprint.HasValue) throw new ArgumentException("Voxel sampler needs its actual footprint", nameof(footprint));
            if (VoxelHeightSampler == null && sampler == null) return;
            var old = voxelHeightFootprint;
            VoxelHeightSampler = sampler;
            voxelHeightFootprint = sampler == null ? null : footprint;
            if (old.HasValue) InvalidateWaterBedBounds(old.Value);
            if (voxelHeightFootprint.HasValue) InvalidateWaterBedBounds(voxelHeightFootprint.Value);
        }

        internal void InvalidateVoxelHeight(Bounds bounds)
        {
            if (VoxelHeightSampler != null) InvalidateWaterBedBounds(bounds);
        }

        private void InvalidateWaterBedBounds(Bounds bounds)
        {
            // Bed normals sample height one metre around each two-metre vertex.
            // Clip absolute samples to the canonical bed, then fold edge stamps
            // so both copies of each periodic boundary are considered.
            int minX = Mathf.Max(0, Mathf.FloorToInt((bounds.min.x - 1) / DetailSize));
            int minZ = Mathf.Max(0, Mathf.FloorToInt((bounds.min.z - 1) / DetailSize));
            int maxX = Mathf.Min(512, Mathf.CeilToInt((bounds.max.x + 1) / DetailSize));
            int maxZ = Mathf.Min(512, Mathf.CeilToInt((bounds.max.z + 1) / DetailSize));
            if (minX > maxX || minZ > maxZ) return;
            ulong next = unchecked(++WaterBedVersion);
            for (int z = minZ; z <= maxZ; z++) for (int x = minX; x <= maxX; x++)
            {
                int wx = x & 511, wz = z & 511;
                waterBedVertexRevisions[wx + wz * 512] = next;
                int sx = wx / DetailCells, sz = wz / DetailCells;
                waterBedRevisions[sx + sz * Sections] = next;
                if (wx % DetailCells == 0) waterBedRevisions[((sx + Sections - 1) % Sections) + sz * Sections] = next;
                if (wz % DetailCells == 0) waterBedRevisions[sx + ((sz + Sections - 1) % Sections) * Sections] = next;
                if (wx % DetailCells == 0 && wz % DetailCells == 0)
                    waterBedRevisions[((sx + Sections - 1) % Sections) + ((sz + Sections - 1) % Sections) * Sections] = next;
            }
        }

        internal ModernTerrainMesh(Transform parent, Material terrainMaterial, Material waterMaterial, ModernShoreMask sourceShore = null, bool voxel = false)
        {
            if (parent == null) throw new ArgumentNullException(nameof(parent));
            if (terrainMaterial == null) throw new ArgumentNullException(nameof(terrainMaterial));
            if (waterMaterial == null) throw new ArgumentNullException(nameof(waterMaterial));
            root = new GameObject("Modern Terrain").transform;
            this.voxel = voxel;
            this.sourceShore=sourceShore;
            root.SetParent(parent, false);
            materials = new[] { terrainMaterial, waterMaterial };
            for (var i = 0; i < sections.Length; i++)
            {
                var mesh = new Mesh { name = $"Modern terrain section {i}" };
                mesh.MarkDynamic();
                var farMesh = new Mesh { name = $"Modern terrain section {i} far" };
                farMesh.MarkDynamic();
                sections[i] = new Section { Mesh = mesh, FarMesh = farMesh,
                    ShoreMesh = new Mesh { name = $"Modern shore overlap {i}" } };
            }
        }

        // The caller supplies all three planes from the same completed simulation snapshot.
        internal void Update(byte[] nextHeights, byte[] nextTypes, byte[] nextAngles, ulong generation, ulong tick)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernTerrainMesh));
            ValidatePlane(nextHeights, nameof(nextHeights));
            ValidatePlane(nextTypes, nameof(nextTypes));
            ValidatePlane(nextAngles, nameof(nextAngles));
            var reset = !initialized || generation != lastGeneration;
            var changed = reset;
            ulong nextBedVersion = unchecked(WaterBedVersion + 1);
            if (reset)
            {
                Array.Copy(nextHeights, heights, PlaneSize);
                Array.Copy(nextTypes, types, PlaneSize);
                Array.Copy(nextAngles, angles, PlaneSize);
                Array.Fill(landformDirty,true);
                for (var i = 0; i < dirty.Length; i++) dirty[i] = true;
            }
            else
            {
                for (var i = 0; i < PlaneSize; i++)
                {
                    var heightChanged = heights[i] != nextHeights[i];
                    var typeChanged = types[i] != nextTypes[i];
                    var waterChanged = ((angles[i] ^ nextAngles[i]) & (sourceShore != null ? 0x78 : 8)) != 0;
                    if (!heightChanged && !typeChanged && !waterChanged)
                    {
                        angles[i] = nextAngles[i];
                        continue;
                    }
                    var x = i & 255;
                    var z = i >> 8;
                    changed = true;
                    heights[i] = nextHeights[i];
                    types[i] = nextTypes[i];
                    angles[i] = nextAngles[i];
                    if(heightChanged || waterChanged || typeChanged)
                        for(var dz=-3;dz<=3;dz++) for(var dx=-3;dx<=3;dx++)
                            landformDirty[Index(x+dx,z+dz)]=true;
                    // Reconstruction has a two-cell kernel and a three-cell shore
                    // protection halo; interpolation/normals need one more cell.
                    for (var dz = -4; dz <= 4; dz++)
                        for (var dx = -4; dx <= 4; dx++)
                            MarkSection(x + dx, z + dz);
                    // Bed heights sample one metre beyond tile edges for normals.
                    // Including detail interpolation, the landform gradient and
                    // reconstruction kernel needs at most six source cells of
                    // influence. Mark the bed sections directly instead of
                    // expanding every dirty terrain section to nine sections.
                    for (var dz = -6; dz <= 6; dz++)
                        for (var dx = -6; dx <= 6; dx++)
                            waterBedRevisions[((x + dx) & 255) / SectionCells
                                + (((z + dz) & 255) / SectionCells) * Sections] = nextBedVersion;
                    // Bed vertices are spaced at half a source cell. Keep the
                    // same six-cell dependency halo at vertex granularity so
                    // a small edit need not resample an entire populated tile.
                    for (var dz = -12; dz <= 12; dz++)
                        for (var dx = -12; dx <= 12; dx++)
                            waterBedVertexRevisions[((x * 2 + dx) & 511)
                                + (((z * 2 + dz) & 511) * 512)] = nextBedVersion;
                }
            }
            lastGeneration = generation;
            initialized = true;
            if (changed) unchecked { Revision++; }
            if (changed) WaterBedVersion = nextBedVersion;
            if (reset)
            {
                unchecked { WaterBedResetRevision++; }
                Array.Fill(waterBedRevisions, WaterBedVersion);
                Array.Fill(waterBedVertexRevisions, WaterBedVersion);
            }
            if (changed) sourceShore?.Update(heights,types,angles,Revision);
            if (changed) ReconstructLandform();
            if (changed) Array.Clear(detailHeightValid,0,detailHeightValid.Length);
            for (var i = 0; i < sections.Length; i++)
            {
                if (!dirty[i]) continue;
                var section = sections[i];
                section.HasWater = RebuildSection(i % Sections, i / Sections, section.Mesh, section.FarMesh, section.ShoreMesh,
                    out section.WaterBounds);
                section.VoxelDirty = section.VoxelFarDirty = true;
                section.DetailTriangles = (int)((section.Mesh.GetIndexCount(0) + section.Mesh.GetIndexCount(1)) / 3);
                section.FarTriangles = (int)((section.FarMesh.GetIndexCount(0) + section.FarMesh.GetIndexCount(1)) / 3);
                dirty[i] = false;
            }
        }

        // Conservative bounds include choppy horizontal waves and local ripples.
        // Present must have been called for the camera's current position first.
        internal bool WaterVisible(Camera camera)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernTerrainMesh));
            if (camera == null) throw new ArgumentNullException(nameof(camera));
            if (!initialized) return false;
            GeometryUtility.CalculateFrustumPlanes(camera, frustumPlanes);
            var wave = materials[1].HasProperty(WaveStrength) ? Mathf.Abs(materials[1].GetFloat(WaveStrength)) : 0f;
            foreach (var section in sections)
            {
                if (!section.HasWater) continue;
                foreach (var copy in section.Copies)
                {
                    if (copy == null || !copy.Object.activeInHierarchy) continue;
                    var matrix = copy.Transform.localToWorldMatrix;
                    var local = section.WaterBounds;
                    var x = matrix.MultiplyVector(new Vector3(local.extents.x, 0f, 0f));
                    var y = matrix.MultiplyVector(new Vector3(0f, local.extents.y, 0f));
                    var z = matrix.MultiplyVector(new Vector3(0f, 0f, local.extents.z));
                    var extents = new Vector3(Mathf.Abs(x.x) + Mathf.Abs(y.x) + Mathf.Abs(z.x),
                        Mathf.Abs(x.y) + Mathf.Abs(y.y) + Mathf.Abs(z.y),
                        Mathf.Abs(x.z) + Mathf.Abs(y.z) + Mathf.Abs(z.z));
                    var bounds = new Bounds(matrix.MultiplyPoint3x4(local.center), extents * 2f);
                    bounds.Expand(new Vector3(wave * 2f, (wave + 2.15f) * 2f, wave * 2f));
                    if (GeometryUtility.TestPlanesAABB(frustumPlanes, bounds)) return true;
                }
            }
            return false;
        }

        // Create only copies within range; section meshes are shared by all copies.
        internal void Present(Vector3 cameraPosition)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernTerrainMesh));
            if (!initialized) return;
            VisibleTriangleCount = 0;
            VisibleDetailCopies = 0;
            VisibleFarCopies = 0;
            var worldX = Mathf.FloorToInt(cameraPosition.x / Period);
            var worldZ = Mathf.FloorToInt(cameraPosition.z / Period);
            presentedWorldX = worldX; presentedWorldZ = worldZ; presented = true;
            for (var sz = 0; sz < Sections; sz++)
            for (var sx = 0; sx < Sections; sx++)
            {
                var section = sections[sx + sz * Sections];
                for (var dz = -1; dz <= 1; dz++)
                for (var dx = -1; dx <= 1; dx++)
                {
                    var slot = (dx + 1) + (dz + 1) * 3;
                    var x = (worldX + dx) * Period + sx * SectionSize;
                    var z = (worldZ + dz) * Period + sz * SectionSize;
                    var nearestX = Mathf.Clamp(cameraPosition.x, x, x + SectionSize);
                    var nearestZ = Mathf.Clamp(cameraPosition.z, z, z + SectionSize);
                    var deltaX = cameraPosition.x - nearestX;
                    var deltaZ = cameraPosition.z - nearestZ;
                    var radius = Mathf.Clamp(ViewDistance, 0f, Period);
                    var visible = deltaX * deltaX + deltaZ * deltaZ <= radius * radius;
                    var copy = section.Copies[slot];
                    if (visible && copy == null)
                    {
                        var gameObject = new GameObject($"Terrain {sx},{sz} copy {slot}");
                        var transform = gameObject.transform;
                        transform.SetParent(root, false);
                        var filter = gameObject.AddComponent<MeshFilter>();
                        filter.sharedMesh = section.Mesh;
                        var renderer = gameObject.AddComponent<MeshRenderer>();
                        renderer.sharedMaterials = materials;
                        var shoreObject=new GameObject("Shore overlap");shoreObject.transform.SetParent(transform,false);
                        shoreObject.AddComponent<MeshFilter>().sharedMesh=section.ShoreMesh;
                        var shoreRenderer=shoreObject.AddComponent<MeshRenderer>();shoreRenderer.sharedMaterial=materials[1];
                        shoreRenderer.shadowCastingMode=ShadowCastingMode.Off;
                        copy = new Copy { Object = gameObject, Transform = transform, Filter = filter,
                            Renderer = renderer, ShoreRenderer=shoreRenderer, Detail = true };
                        section.Copies[slot] = copy;
                    }
                    if (copy == null) continue;
                    if (copy.Object.activeSelf != visible) copy.Object.SetActive(visible);
                    if (visible)
                    {
                        copy.Transform.localPosition = new Vector3(x, 0f, z);
                        copy.ShoreRenderer.enabled=section.ShoreMesh.GetIndexCount(0)>0;
                        var threshold = copy.Detail ? DetailExitDistance : DetailEnterDistance;
                        // Visibility remains a horizontal wrap window; LOD uses
                        // the nearest 3D bounds point, including flight altitude.
                        var detail = copy.Renderer.bounds.SqrDistance(cameraPosition) <= threshold * threshold;
                        if (voxel && PatchTouchesSection(sx, sz)) detail = true;
                        Mesh selected;
                        if (voxel)
                        {
                            selected = VoxelMesh(section, sx, sz, detail);
                        }
                        else selected = detail ? section.Mesh : section.FarMesh;
                        if (copy.Filter.sharedMesh != selected) copy.Filter.sharedMesh = selected;
                        copy.Detail = detail;
                        VisibleTriangleCount += voxel ? (int)((selected.GetIndexCount(0) + selected.GetIndexCount(1)) / 3) :
                            detail ? section.DetailTriangles : section.FarTriangles;
                        VisibleTriangleCount += (int)section.ShoreMesh.GetIndexCount(0)/3;
                        if (detail) VisibleDetailCopies++; else VisibleFarCopies++;
                    }
                }
            }
        }

        private bool PatchTouchesSection(int sx, int sz)
        {
            if (!VoxelPatch.HasValue) return false;
            var patch = VoxelPatch.Value;
            float x = sx * SectionSize, z = sz * SectionSize;
            return x < patch.max.x + 4f && x + SectionSize > patch.min.x - 4f &&
                   z < patch.max.z + 4f && z + SectionSize > patch.min.z - 4f;
        }

        private Mesh VoxelMesh(Section section, int sx, int sz, bool detail)
        {
            var mesh = detail ? section.VoxelMesh : section.VoxelFarMesh;
            if (mesh == null)
            {
                mesh = new Mesh { name = $"Voxel terrain section {sx},{sz} {(detail ? "near" : "far")}" };
                mesh.MarkDynamic();
                if (detail) section.VoxelMesh = mesh; else section.VoxelFarMesh = mesh;
            }
            if (detail ? section.VoxelDirty : section.VoxelFarDirty)
            {
                Voxels.VoxelTerrainSurface.Build(mesh, detail ? section.Mesh : section.FarMesh,
                    new Vector2(sx * SectionSize, sz * SectionSize),
                    detail ? Voxels.VoxelTerrainSurface.NearCell : Voxels.VoxelTerrainSurface.FarCell,
                    VisualSampleHeight, (x, z) => Water(Mathf.FloorToInt(x / CellSize), Mathf.FloorToInt(z / CellSize)),
                    (x, z) => DetailColor(Mathf.FloorToInt(x / DetailSize), Mathf.FloorToInt(z / DetailSize)), VoxelPatch);
                if (detail) section.VoxelDirty = false; else section.VoxelFarDirty = false;
            }
            return mesh;
        }

        // Interpolate the same half-cell triangles used by the rendered mesh.
        // The source collision sampler has a distinct 255 rule.
        internal float SampleHeight(float x, float z)
        {
            if(VoxelHeightSampler!=null&&VoxelHeightSampler(x,z) is float local)return local;
            return VisualSampleHeight(x, z);
        }

        // Presentation roots can follow the actually selected exterior-column
        // LOD without changing source collision, water-bed or local patch state.
        // When no matching copy is currently presented, retain the old sampler.
        internal float SampleVisualHeight(float x, float z)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernTerrainMesh));
            if (!voxel || !initialized || !presented || IsWater(x, z)) return SampleHeight(x, z);
            float wx = Mathf.Repeat(x, Period), wz = Mathf.Repeat(z, Period);
            if (Voxels.VoxelTerrainSurface.InPatchCollar(wx, wz, VoxelPatch)) return SampleHeight(x, z);
            int sx = Mathf.FloorToInt(wx / SectionSize), sz = Mathf.FloorToInt(wz / SectionSize);
            int dx = Mathf.FloorToInt(x / Period) - presentedWorldX;
            int dz = Mathf.FloorToInt(z / Period) - presentedWorldZ;
            if (dx < -1 || dx > 1 || dz < -1 || dz > 1) return SampleHeight(x, z);
            var section = sections[sx + sz * Sections];
            var copy = section.Copies[(dx + 1) + (dz + 1) * 3];
            if (copy == null || !copy.Object.activeSelf) return SampleHeight(x, z);
            // Observation can mutate terrain before the next presentation pass.
            // Refresh the selected mesh before reporting a placement on it.
            VoxelMesh(section, sx, sz, copy.Detail);
            return Voxels.VoxelTerrainSurface.SurfaceHeight(wx, wz,
                copy.Detail ? Voxels.VoxelTerrainSurface.NearCell : Voxels.VoxelTerrainSurface.FarCell,
                VisualSampleHeight);
        }

        private float VisualSampleHeight(float x, float z)
        {
            if (!initialized) return 0f;
            var wx = Mathf.Repeat(x, Period) / DetailSize;
            var wz = Mathf.Repeat(z, Period) / DetailSize;
            var ix = Mathf.FloorToInt(wx);
            var iz = Mathf.FloorToInt(wz);
            var fx = wx - ix;
            var fz = wz - iz;
            return TriangleHeight(DetailHeight(ix, iz), DetailHeight(ix + 1, iz),
                DetailHeight(ix, iz + 1), DetailHeight(ix + 1, iz + 1),
                fx, fz, ((ix / Subdivisions + iz / Subdivisions) & 1) == 0);
        }

        internal bool IsWater(float x, float z)
        {
            if (!initialized) return false;
            var ix = Mathf.FloorToInt(Mathf.Repeat(x, Period) / CellSize);
            var iz = Mathf.FloorToInt(Mathf.Repeat(z, Period) / CellSize);
            return Water(ix,iz);
        }

        internal byte GroundType(float x, float z)
        {
            if (!initialized) return 0;
            var ix = Mathf.FloorToInt(Mathf.Repeat(x, Period) / CellSize);
            var iz = Mathf.FloorToInt(Mathf.Repeat(z, Period) / CellSize);
            return types[Index(ix, iz)];
        }

        // The source reflection table is independent of the outdoor wave bit.
        internal static bool IsReflectiveMaterial(byte type)
        {
            return type == 0 || (type >= 35 && type <= 39)
                || (type >= 44 && type <= 47) || (type >= 80 && type <= 83)
                || (type >= 102 && type <= 119) || type == 126 || type == 127;
        }

        private bool RebuildSection(int sx, int sz, Mesh mesh, Mesh farMesh, Mesh shoreMesh, out Bounds waterBounds)
        {
            var vertices = new Vector3[Side * Side];
            var normals = new Vector3[vertices.Length];
            var colors = new Color[vertices.Length];
            var uv = new Vector2[vertices.Length];
            var land = new List<int>(DetailCells * DetailCells * 6);
            var water = new List<int>();
            var farLand = new List<int>();
            var farWater = new List<int>();
            var shore=new List<int>();
            var baseX = sx * SectionCells;
            var baseZ = sz * SectionCells;
            var fine = new bool[SectionCells * SectionCells];
            for (var z = 0; z < SectionCells; z++)
            for (var x = 0; x < SectionCells; x++)
                fine[x + z * SectionCells] = FineCell(x, z, baseX, baseZ);
            for (var z = 0; z < Side; z++)
            for (var x = 0; x < Side; x++)
            {
                var ix = baseX * Subdivisions + x;
                var iz = baseZ * Subdivisions + z;
                var v = x + z * Side;
                vertices[v] = new Vector3(x * DetailSize, DetailHeight(ix, iz), z * DetailSize);
                // Wrapped halo gives identical normals at every section and map seam.
                normals[v] = new Vector3(DetailHeight(ix - 1, iz) - DetailHeight(ix + 1, iz),
                    2f * DetailSize, DetailHeight(ix, iz - 1) - DetailHeight(ix, iz + 1)).normalized;
                colors[v] = DetailColor(ix, iz);
                uv[v] = new Vector2(ix * DetailSize, iz * DetailSize);
            }
            for (var z = 0; z < DetailCells; z++)
            for (var x = 0; x < DetailCells; x++)
            {
                var a = x + z * Side;
                var b = a + 1;
                var c = a + Side;
                var d = c + 1;
                var sourceX = baseX + x / Subdivisions;
                var sourceZ = baseZ + z / Subdivisions;
                var target = Water(sourceX, sourceZ) ? water : land;
                var farTarget = target == water ? farWater : farLand;
                var refined = fine[x / Subdivisions + z / Subdivisions * SectionCells];
                var shoreCell=false;
                if(target==land)
                {
                    shoreCell=sourceShore!=null && heights[Index(sourceX,sourceZ)]==0
                        && sourceShore.HasCoverage(types[Index(sourceX,sourceZ)]);
                    for(var dz=-1;dz<=1;dz++) for(var dx=-1;dx<=1;dx++)
                        shoreCell |= Water(sourceX+dx,sourceZ+dz);
                }
                if (((sourceX + sourceZ) & 1) == 0)
                {
                    target.Add(a); target.Add(c); target.Add(d);
                    target.Add(a); target.Add(d); target.Add(b);
                    if(shoreCell) { shore.Add(a);shore.Add(c);shore.Add(d);shore.Add(a);shore.Add(d);shore.Add(b); }
                    if (refined)
                    {
                        farTarget.Add(a); farTarget.Add(c); farTarget.Add(d);
                        farTarget.Add(a); farTarget.Add(d); farTarget.Add(b);
                    }
                }
                else
                {
                    target.Add(a); target.Add(c); target.Add(b);
                    target.Add(c); target.Add(d); target.Add(b);
                    if(shoreCell) { shore.Add(a);shore.Add(c);shore.Add(b);shore.Add(c);shore.Add(d);shore.Add(b); }
                    if (refined)
                    {
                        farTarget.Add(a); farTarget.Add(c); farTarget.Add(b);
                        farTarget.Add(c); farTarget.Add(d); farTarget.Add(b);
                    }
                }
            }
            for (var z = 1; z < SectionCells - 1; z++)
            for (var x = 1; x < SectionCells - 1; x++)
            {
                var cell = x + z * SectionCells;
                if (fine[cell]) continue;
                var target = Water(baseX + x, baseZ + z) ? farWater : farLand;
                var a = x * 2 + z * 2 * Side;
                var b = a + 2;
                var c = a + 2 * Side;
                var d = c + 2;
                var left = fine[cell - 1];
                var right = fine[cell + 1];
                var top = fine[cell - SectionCells];
                var bottom = fine[cell + SectionCells];
                if (!(left || right || top || bottom))
                {
                    if (((baseX + x + baseZ + z) & 1) == 0)
                    {
                        target.Add(a); target.Add(c); target.Add(d);
                        target.Add(a); target.Add(d); target.Add(b);
                    }
                    else
                    {
                        target.Add(a); target.Add(c); target.Add(b);
                        target.Add(c); target.Add(d); target.Add(b);
                    }
                    continue;
                }
                // Match every detailed neighbour's midpoint, including relief and
                // shore geometry. The center fan keeps each shared edge identical.
                var center = a + Side + 1;
                var previous = a;
                if (left) AddFan(target, center, ref previous, a + Side);
                AddFan(target, center, ref previous, c);
                if (bottom) AddFan(target, center, ref previous, c + 1);
                AddFan(target, center, ref previous, d);
                if (right) AddFan(target, center, ref previous, b + Side);
                AddFan(target, center, ref previous, b);
                if (top) AddFan(target, center, ref previous, a + 1);
                AddFan(target, center, ref previous, a);
            }
            if(VoxelPatch.HasValue)
            {
                var patch=VoxelPatch.Value;
                void Cut(List<int> list) {
                    int write=0;
                    for(int read=0;read<list.Count;read+=3){
                        var p=(vertices[list[read]]+vertices[list[read+1]]+vertices[list[read+2]])/3+new Vector3(baseX*CellSize,0,baseZ*CellSize);
                        if(p.x>patch.min.x&&p.x<patch.max.x&&p.z>patch.min.z&&p.z<patch.max.z)continue;
                        list[write++]=list[read];list[write++]=list[read+1];list[write++]=list[read+2];
                    }
                    if(write<list.Count)list.RemoveRange(write,list.Count-write);
                }
                Cut(land);Cut(farLand);Cut(shore);
            }
            SetMesh(mesh, vertices, normals, colors, uv, land, water);
            SetMesh(farMesh, vertices, normals, colors, uv, farLand, farWater);
            shoreMesh.Clear();shoreMesh.vertices=vertices;shoreMesh.normals=normals;
            shoreMesh.colors=colors;shoreMesh.uv=uv;shoreMesh.SetTriangles(shore,0);shoreMesh.RecalculateBounds();
            var shoreBounds=shoreMesh.bounds;var datumCenter=shoreBounds.center;datumCenter.y=.02f;
            shoreBounds.Encapsulate(datumCenter);shoreBounds.Expand(new Vector3(6,10.3f,6));shoreMesh.bounds=shoreBounds;
            var hasWater=water.Count!=0 || shore.Count!=0;
            var first=hasWater?vertices[water.Count>0?water[0]:shore[0]]:Vector3.zero;first.y=.02f;
            waterBounds=new Bounds(first,Vector3.zero);
            foreach(var index in water) { var v=vertices[index];v.y=.02f;waterBounds.Encapsulate(v); }
            foreach(var index in shore) { var v=vertices[index];v.y=.02f;waterBounds.Encapsulate(v); }
            return hasWater;
        }

        private bool FineCell(int x, int z, int baseX, int baseZ)
        {
            if (x == 0 || z == 0 || x == SectionCells - 1 || z == SectionCells - 1) return true;
            var index=Index(baseX+x,baseZ+z);
            if(sourceShore!=null && types[index]!=0 && heights[index]==0 && sourceShore.HasCoverage(types[index])) return true;
            var water = Water(baseX + x, baseZ + z);
            // Both sides of a shoreline, including diagonal contacts, retain
            // detailed geometry and the original interpolated shore colors.
            for (var dz = -1; dz <= 1; dz++)
            for (var dx = -1; dx <= 1; dx++)
                if (Water(baseX + x + dx, baseZ + z + dz) != water) return true;
            return false;
        }

        private static void AddFan(List<int> triangles, int center, ref int previous, int next)
        {
            triangles.Add(center); triangles.Add(previous); triangles.Add(next);
            previous = next;
        }

        private static void SetMesh(Mesh mesh, Vector3[] vertices, Vector3[] normals, Color[] colors,
            Vector2[] uv, List<int> land, List<int> water)
        {
            mesh.Clear();
            mesh.vertices = vertices;
            mesh.normals = normals;
            mesh.colors = colors;
            mesh.uv = uv;
            mesh.subMeshCount = 2;
            mesh.SetTriangles(land, 0, false);
            mesh.SetTriangles(water, 1, false);
            mesh.RecalculateBounds();
            if (water.Count != 0)
            {
                var bounds = mesh.bounds;
                var datum=bounds.center;datum.y=.02f;bounds.Encapsulate(datum);
                bounds.Expand(new Vector3(6, 10.3f, 6));
                mesh.bounds = bounds;
            }
        }

        private static void ValidatePlane(byte[] plane, string name)
        {
            if (plane == null || plane.Length != PlaneSize)
                throw new ArgumentException("Terrain plane must contain exactly 65536 bytes.", name);
        }

        private void MarkSection(int x, int z)
        {
            dirty[((x & 255) / SectionCells) + ((z & 255) / SectionCells) * Sections] = true;
        }

        private static int Index(int x, int z) => (x & 255) + (z & 255) * MapCells;
        private float Height(int x, int z) => heights[Index(x, z)] * 0.5f;
        private float VisualHeight(int x, int z) => landform[Index(x,z)];
        // The source wave bit is only set in the eroded interior of a water
        // region (Terrain.cpp sub_43D50). Calm sea-level type-0 tiles are water
        // too (sub_104D0), not rectangular sand platforms along the coast.
        private bool Water(int x, int z)
        {
            var i=Index(x,z);
            // Modern sea-level transition tiles become continuous submerged
            // banks. Original opaque texture pixels are not raised sand islands.
            // Elevated land and authoritative Rust water/collision stay intact.
            return (angles[i]&8)!=0 || (heights[i]==0 && (types[i]==0
                || (sourceShore!=null && sourceShore.HasCoverage(types[i]))));
        }

        // Source collision stays in Rust. The Modern surface reconstructs the
        // quantized four-metre heightfield as a continuous eroded landform rather
        // than extruding its one-cell ridges. The near-flat bank extends below
        // the water datum so receding waves cannot expose coplanar grid ledges.
        private void ReconstructLandform()
        {
            for(var z=0;z<MapCells;z++) for(var x=0;x<MapCells;x++)
            {
                if(!landformDirty[Index(x,z)]) continue;
                landformDirty[Index(x,z)]=false;
                var original=Height(x,z);
                var nearest=4f;
                for(var dz=-3;dz<=2;dz++) for(var dx=-3;dx<=2;dx++)
                    if((angles[Index(x+dx,z+dz)]&8)!=0)
                    {
                        var sx=Mathf.Max(0,Mathf.Max(dx,-dx-1));
                        var sz=Mathf.Max(0,Mathf.Max(dz,-dz-1));
                        nearest=Mathf.Min(nearest,Mathf.Sqrt(sx*sx+sz*sz));
                    }
                var shore=Mathf.SmoothStep(0,1,Mathf.Clamp01(nearest/3));
                // Only interior-water anchors constrain reconstruction. Mixed
                // coastal tiles must join the smooth landform rather than pinning
                // each zero-height texel beside a raised cliff to the sea datum.
                var bankDistance=nearest;
                if(sourceShore!=null)
                    for(var dz=-1;dz<=0;dz++) for(var dx=-1;dx<=0;dx++)
                        if(heights[Index(x+dx,z+dz)]==0 && sourceShore.HasCoverage(types[Index(x+dx,z+dz)])) bankDistance=0;
                var bankDrop=.45f*(1-Mathf.SmoothStep(0,1,Mathf.Clamp01(bankDistance)))
                    *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(1,3,original)));
                if(shore==0) { landform[Index(x,z)]=original-bankDrop; continue; }
                var average=0f;
                for(var dz=-2;dz<=2;dz++) for(var dx=-2;dx<=2;dx++)
                {
                    var wx=dx==0?6:Math.Abs(dx)==1?4:1;
                    var wz=dz==0?6:Math.Abs(dz)==1?4:1;
                    average+=Height(x+dx,z+dz)*wx*wz;
                }
                // Convex filtering preserves the broad relief and cannot invent
                // a higher peak or lower pit. Fade near the actual water mask.
                landform[Index(x,z)]=Mathf.Lerp(original,average/256,.88f*shore)-bankDrop;
            }
        }

        private Color SourceColor(int x, int z)
        {
            var variation = (types[Index(x, z)] % 7) * 0.015f;
            return new Color(0.92f + variation, 0.95f + variation * 0.5f,
                0.90f + variation, Shore(x, z) ? 1f : 0f);
        }

        // Shared by the submerged extension at the same two-metre lattice vertices.
        internal Color DetailColor(int x, int z)
        {
            var ix = x >> 1;
            var iz = z >> 1;
            var oddX = (x & 1) != 0;
            var oddZ = (z & 1) != 0;
            if (!oddX && !oddZ) return SourceColor(ix, iz);
            if (!oddX) return (SourceColor(ix, iz) + SourceColor(ix, iz + 1)) * 0.5f;
            if (!oddZ) return (SourceColor(ix, iz) + SourceColor(ix + 1, iz)) * 0.5f;
            return ((ix + iz) & 1) == 0
                ? (SourceColor(ix, iz) + SourceColor(ix + 1, iz + 1)) * 0.5f
                : (SourceColor(ix + 1, iz) + SourceColor(ix, iz + 1)) * 0.5f;
        }

        private static float TriangleHeight(float h00, float h10, float h01, float h11,
            float fx, float fz, bool even)
        {
            if (even)
                return fx <= fz
                    ? h00 + fz * (h01 - h00) + fx * (h11 - h01)
                    : h00 + fz * (h11 - h10) + fx * (h10 - h00);
            return fx + fz >= 1f
                ? h01 + (1f - fz) * (h10 - h11) + fx * (h11 - h01)
                : h00 + fz * (h01 - h00) + fx * (h10 - h00);
        }

        // Half-grid coordinates are wrapped by the source lookups and detail hash.
        // Arithmetic shifts floor negative coordinates, including the seam halo.
        private float BaseHalfHeight(int x, int z)
        {
            var ix = x >> 1;
            var iz = z >> 1;
            return TriangleHeight(VisualHeight(ix, iz), VisualHeight(ix + 1, iz),
                VisualHeight(ix, iz + 1), VisualHeight(ix + 1, iz + 1),
                (x & 1) * 0.5f, (z & 1) * 0.5f, ((ix + iz) & 1) == 0);
        }

        private float DetailHeight(int x, int z)
        {
            // Mesh construction and every hoof/contact sample consume the same
            // periodic height lattice. Relief depends only on the current
            // terrain snapshot, so don't recompute its noise per contact point.
            int key=(x&511)+(z&511)*512;
            if(!detailHeightValid[key]){detailHeights[key]=ComputeDetailHeight(x,z);detailHeightValid[key]=true;}
            return detailHeights[key];
        }

        private float ComputeDetailHeight(int x, int z)
        {
            var original = BaseHalfHeight(x, z);
            var ix = x >> 1;
            var iz = z >> 1;
            // A midpoint touching any water cell stays on the original surface,
            // so the water and land meshes meet without raising shorelines.
            if (Water(ix, iz) || ((x & 1) == 0 && Water(ix - 1, iz))
                || ((z & 1) == 0 && Water(ix, iz - 1))
                || (((x|z)&1)==0 && Water(ix-1,iz-1))) return original;
            // Coherent metre-scale outcrops break the filtered cones without
            // adding independent spikes at every vertex. Fade on level ground,
            // near water, and at local extrema so the broad relief stays legible.
            var gradient=Mathf.Abs(VisualHeight(ix-1,iz)-VisualHeight(ix+1,iz))
                +Mathf.Abs(VisualHeight(ix,iz-1)-VisualHeight(ix,iz+1));
            var strength=Mathf.SmoothStep(0,1,Mathf.Clamp01((original-3)/10))
                *Mathf.Clamp01(gradient/8);
            var outcrop=((ReliefNoise(x,z,16)-.5f)*2.4f+(ReliefNoise(x+51,z-29,4)-.5f)*.9f)*strength;
            original+=outcrop;
            if (((x | z) & 1) == 0) return original;
            var average = (BaseHalfHeight(x - 1, z) + BaseHalfHeight(x + 1, z)
                + BaseHalfHeight(x, z - 1) + BaseHalfHeight(x, z + 1)) * 0.25f;
            var smooth = Mathf.Clamp((average - original) * 0.15f, -0.07f, 0.07f);
            // Fixed periodic relief: deterministic across chunks, snapshots and wraps.
            uint hash = (uint)(x & 511) * 0x8da6b343u ^ (uint)(z & 511) * 0xd8163841u;
            hash ^= hash >> 13;
            hash *= 0x85ebca6bu;
            hash ^= hash >> 16;
            var relief = ((hash & 1023u) / 1023f * 2f - 1f) * 0.08f;
            return original + smooth + relief;
        }

        private static float ReliefNoise(int x,int z,int scale)
        {
            var fx=x/(float)scale;var fz=z/(float)scale;
            var ix=Mathf.FloorToInt(fx);var iz=Mathf.FloorToInt(fz);
            fx-=ix;fz-=iz;fx=fx*fx*(3-2*fx);fz=fz*fz*(3-2*fz);
            var period=512/scale;
            float Value(int a,int b)
            {
                uint h=(uint)(a&(period-1))*0x8da6b343u^(uint)(b&(period-1))*0xd8163841u;
                h^=h>>13;h*=0x85ebca6bu;h^=h>>16;
                return (h&65535)/65535f;
            }
            return Mathf.Lerp(Mathf.Lerp(Value(ix,iz),Value(ix+1,iz),fx),
                Mathf.Lerp(Value(ix,iz+1),Value(ix+1,iz+1),fx),fz);
        }
        private bool Shore(int x, int z)
        {
            var count = (Water(x, z) ? 1 : 0) + (Water(x - 1, z) ? 1 : 0)
                + (Water(x, z - 1) ? 1 : 0) + (Water(x - 1, z - 1) ? 1 : 0);
            return count > 0 && count < 4;
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            foreach (var section in sections)
            {
                Destroy(section.Mesh);
                Destroy(section.FarMesh);
                Destroy(section.ShoreMesh);
                Destroy(section.VoxelMesh);
                Destroy(section.VoxelFarMesh);
            }
            Destroy(root.gameObject);
        }

        private static void Destroy(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }
    }
}
