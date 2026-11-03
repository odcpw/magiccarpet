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

        private sealed class Section
        {
            internal Mesh Mesh;
            internal readonly GameObject[] Copies = new GameObject[9];
        }

        private readonly Transform root;
        private readonly Material[] materials;
        private readonly Section[] sections = new Section[Sections * Sections];
        private readonly byte[] heights = new byte[PlaneSize];
        private readonly byte[] types = new byte[PlaneSize];
        private readonly byte[] angles = new byte[PlaneSize];
        private readonly bool[] dirty = new bool[Sections * Sections];
        private bool initialized;
        private bool disposed;
        private ulong lastGeneration;
        internal ulong Revision { get; private set; }
        // Values up to one world period are supported by the 3x3 copy window.
        internal float ViewDistance { get; set; } = 320f;

        internal ModernTerrainMesh(Transform parent, Material terrainMaterial, Material waterMaterial)
        {
            if (parent == null) throw new ArgumentNullException(nameof(parent));
            if (terrainMaterial == null) throw new ArgumentNullException(nameof(terrainMaterial));
            if (waterMaterial == null) throw new ArgumentNullException(nameof(waterMaterial));
            root = new GameObject("Modern Terrain").transform;
            root.SetParent(parent, false);
            materials = new[] { terrainMaterial, waterMaterial };
            for (var i = 0; i < sections.Length; i++)
            {
                var mesh = new Mesh { name = $"Modern terrain section {i}" };
                mesh.MarkDynamic();
                sections[i] = new Section { Mesh = mesh };
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
            if (reset)
            {
                Array.Copy(nextHeights, heights, PlaneSize);
                Array.Copy(nextTypes, types, PlaneSize);
                Array.Copy(nextAngles, angles, PlaneSize);
                for (var i = 0; i < dirty.Length; i++) dirty[i] = true;
            }
            else
            {
                for (var i = 0; i < PlaneSize; i++)
                {
                    var heightChanged = heights[i] != nextHeights[i];
                    var typeChanged = types[i] != nextTypes[i];
                    var waterChanged = ((angles[i] ^ nextAngles[i]) & 8) != 0;
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
                    // Detail samples smooth adjacent half-cells, and normals use
                    // another half-cell halo. Two source cells cover both and shore.
                    for (var dz = -2; dz <= 2; dz++)
                        for (var dx = -2; dx <= 2; dx++)
                            MarkSection(x + dx, z + dz);
                }
            }
            lastGeneration = generation;
            initialized = true;
            if (changed) unchecked { Revision++; }
            for (var i = 0; i < sections.Length; i++)
            {
                if (!dirty[i]) continue;
                RebuildSection(i % Sections, i / Sections, sections[i].Mesh);
                dirty[i] = false;
            }
        }

        // Create only copies within range; section meshes are shared by all copies.
        internal void Present(Vector3 cameraPosition)
        {
            if (disposed) throw new ObjectDisposedException(nameof(ModernTerrainMesh));
            if (!initialized) return;
            var worldX = Mathf.FloorToInt(cameraPosition.x / Period);
            var worldZ = Mathf.FloorToInt(cameraPosition.z / Period);
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
                        copy = new GameObject($"Terrain {sx},{sz} copy {slot}");
                        copy.transform.SetParent(root, false);
                        copy.AddComponent<MeshFilter>().sharedMesh = section.Mesh;
                        var renderer = copy.AddComponent<MeshRenderer>();
                        renderer.sharedMaterials = materials;
                        section.Copies[slot] = copy;
                    }
                    if (copy == null) continue;
                    if (copy.activeSelf != visible) copy.SetActive(visible);
                    if (visible) copy.transform.localPosition = new Vector3(x, 0f, z);
                }
            }
        }

        // Interpolate the same half-cell triangles used by the rendered mesh.
        // The source collision sampler has a distinct 255 rule.
        internal float SampleHeight(float x, float z)
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
            return Water(ix, iz);
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

        private void RebuildSection(int sx, int sz, Mesh mesh)
        {
            var vertices = new Vector3[Side * Side];
            var normals = new Vector3[vertices.Length];
            var colors = new Color[vertices.Length];
            var uv = new Vector2[vertices.Length];
            var land = new List<int>(DetailCells * DetailCells * 6);
            var water = new List<int>();
            var baseX = sx * SectionCells;
            var baseZ = sz * SectionCells;
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
                if (((sourceX + sourceZ) & 1) == 0)
                {
                    target.Add(a); target.Add(c); target.Add(d);
                    target.Add(a); target.Add(d); target.Add(b);
                }
                else
                {
                    target.Add(a); target.Add(c); target.Add(b);
                    target.Add(c); target.Add(d); target.Add(b);
                }
            }
            mesh.Clear();
            mesh.vertices = vertices;
            mesh.normals = normals;
            mesh.colors = colors;
            mesh.uv = uv;
            mesh.subMeshCount = 2;
            mesh.SetTriangles(land, 0, false);
            mesh.SetTriangles(water, 1, false);
            mesh.RecalculateBounds();
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
        private bool Water(int x, int z) => (angles[Index(x, z)] & 8) != 0;

        private Color SourceColor(int x, int z)
        {
            var variation = (types[Index(x, z)] % 7) * 0.015f;
            return new Color(0.92f + variation, 0.95f + variation * 0.5f,
                0.90f + variation, Shore(x, z) ? 1f : 0f);
        }

        private Color DetailColor(int x, int z)
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
            return TriangleHeight(Height(ix, iz), Height(ix + 1, iz),
                Height(ix, iz + 1), Height(ix + 1, iz + 1),
                (x & 1) * 0.5f, (z & 1) * 0.5f, ((ix + iz) & 1) == 0);
        }

        private float DetailHeight(int x, int z)
        {
            var original = BaseHalfHeight(x, z);
            if (((x | z) & 1) == 0) return original;
            var ix = x >> 1;
            var iz = z >> 1;
            // A midpoint touching any water cell stays on the original surface,
            // so the water and land meshes meet without raising shorelines.
            if (Water(ix, iz) || ((x & 1) == 0 && Water(ix - 1, iz))
                || ((z & 1) == 0 && Water(ix, iz - 1))) return original;
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
            foreach (var section in sections) Destroy(section.Mesh);
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
