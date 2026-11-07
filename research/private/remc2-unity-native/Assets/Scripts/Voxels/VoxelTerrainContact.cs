using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native.Voxels
{
    // Moving, non-rendered collision proxy for the unchanged Modern terrain.
    // Retain separate tiles around the camera and active rubble. A camera cut
    // never cooks a huge empty rectangle between the camera and old fragments.
    // Exclusion is conservative: a quad touching the patch interior is omitted.
    // Align the patch to the 2-unit lattice to avoid a surrounding collision gap.
    internal sealed class VoxelTerrainContact : IDisposable
    {
        private const float Step = 2f;
        private const float Tile = 32f;
        private const int Cells = 16;
        private readonly Func<float, float, float> sampleHeight;
        private readonly GameObject root;
        private readonly Dictionary<Vector2Int, MeshCollider> tiles = new();
        private readonly HashSet<Vector2Int> required = new();
        private readonly List<Vector2Int> obsolete = new();
        private Bounds previousExclusion;
        private ulong terrainRevision;
        private bool built, disposed;
        internal int TileCount => tiles.Count;

        internal VoxelTerrainContact(Transform parent, Func<float, float, float> sampleHeight, int layer = 8)
        {
            this.sampleHeight = sampleHeight ?? throw new ArgumentNullException(nameof(sampleHeight));
            if (layer < 0 || layer > 31) throw new ArgumentOutOfRangeException(nameof(layer));
            root = new GameObject("Voxel terrain contact");
            root.layer = layer;
            root.transform.SetParent(parent, true);
        }

        // Returns true only when the collision mesh was rebuilt.
        internal bool Update(Vector3 camera, Bounds excluded, IReadOnlyList<Vector3> debris = null, ulong revision = 0)
        {
            if (disposed) throw new ObjectDisposedException(nameof(VoxelTerrainContact));
            required.Clear();
            void Include(Vector3 position,int low,int high){
                int x=Mathf.FloorToInt(position.x/Tile),z=Mathf.FloorToInt(position.z/Tile);
                for(int j=low;j<=high;j++)for(int i=low;i<=high;i++)required.Add(new Vector2Int(x+i,z+j));
            }
            Include(camera,-2,1);
            if(debris!=null)foreach(var position in debris)Include(position,-1,1);
            bool changed=false,exclusionChanged=built&&previousExclusion!=excluded;
            obsolete.Clear();
            foreach(var tile in tiles)if(exclusionChanged||!required.Contains(tile.Key)||
                terrainRevision!=revision&&HeightChanged(tile.Value.sharedMesh))obsolete.Add(tile.Key);
            foreach(var key in obsolete){Release(tiles[key]);tiles.Remove(key);changed=true;}
            foreach(var key in required)if(!tiles.ContainsKey(key)){tiles.Add(key,BuildTile(key,excluded));changed=true;}
            previousExclusion=excluded;terrainRevision=revision;built=true;
            return changed;
        }

        private bool HeightChanged(Mesh mesh)
        {
            // Source updates are uncommon. Compare the cached exact lattice;
            // retain unchanged tiles even when another part of the world changed.
            foreach(var point in mesh.vertices)if(point.y!=sampleHeight(point.x,point.z))return true;
            return false;
        }

        private MeshCollider BuildTile(Vector2Int tile,Bounds excluded)
        {
            float minX = excluded.min.x, maxX = excluded.max.x, minZ = excluded.min.z, maxZ = excluded.max.z;
            float startX=tile.x*Tile,startZ=tile.y*Tile;
            var vertices = new Vector3[(Cells + 1) * (Cells + 1)];
            var triangles = new List<int>(Cells * Cells * 6);
            for (int j = 0; j <= Cells; j++) for (int i = 0; i <= Cells; i++)
            {
                float wx = startX + i * Step, wz = startZ + j * Step;
                vertices[i + j * (Cells + 1)] = new Vector3(wx, sampleHeight(wx, wz), wz);
            }
            bool hasExclusion = maxX > minX && maxZ > minZ;
            for (int j = 0; j < Cells; j++) for (int i = 0; i < Cells; i++)
            {
                float wx = startX + i * Step, wz = startZ + j * Step;
                if (hasExclusion && wx < maxX && wx + Step > minX && wz < maxZ && wz + Step > minZ) continue;
                int a = i + j * (Cells + 1), b = a + 1, c = a + Cells + 1, d = c + 1;
                // Match ModernTerrainMesh's alternating source-cell diagonals.
                bool even = ((Mathf.FloorToInt(wx / 4f) + Mathf.FloorToInt(wz / 4f)) & 1) == 0;
                if (even) { triangles.Add(a); triangles.Add(c); triangles.Add(d); triangles.Add(a); triangles.Add(d); triangles.Add(b); }
                else { triangles.Add(a); triangles.Add(c); triangles.Add(b); triangles.Add(b); triangles.Add(c); triangles.Add(d); }
            }
            var replacement = new Mesh { name = "Source terrain contact tile", indexFormat = IndexFormat.UInt16 };
            replacement.vertices = vertices;
            replacement.SetTriangles(triangles, 0);
            replacement.RecalculateBounds();
            var child=new GameObject($"Terrain contact {tile.x},{tile.y}");child.layer=root.layer;child.transform.SetParent(root.transform,false);
            var collider=child.AddComponent<MeshCollider>();
            collider.sharedMesh = replacement;
            return collider;
        }

        private static void Release(MeshCollider collider){collider.enabled=false;DestroyObject(collider.sharedMesh);DestroyObject(collider.gameObject);}

        private static void DestroyObject(UnityEngine.Object value)
        {
            if (value == null) return;
            if (Application.isPlaying) UnityEngine.Object.Destroy(value);
            else UnityEngine.Object.DestroyImmediate(value);
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            root.SetActive(false);
            foreach(var collider in tiles.Values)Release(collider);
            tiles.Clear();
            DestroyObject(root);
        }
    }
}
