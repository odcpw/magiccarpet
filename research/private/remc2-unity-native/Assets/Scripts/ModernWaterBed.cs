using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Authored submerged geometry makes shallow transmission see ground rather
    // than the sky clear colour. Uses the same visual coast/depth as the waves;
    // this is not recovered source bathymetry and does not change Rust collision.
    internal sealed class ModernWaterBed : IDisposable
    {
        private const int Cells=64, Side=Cells+1, Tiles=8;
        private readonly GameObject root;
        private readonly Mesh[] meshes=new Mesh[Tiles*Tiles];
        private readonly GameObject[] nodes=new GameObject[Tiles*Tiles];
        private readonly bool[] populated=new bool[Tiles*Tiles];
        private readonly Vector3[][] verticesByTile = new Vector3[Tiles*Tiles][];
        private readonly Vector3[][] normalsByTile = new Vector3[Tiles*Tiles][];
        private readonly Color[][] colorsByTile = new Color[Tiles*Tiles][];
        private readonly ulong[][] vertexRevisions = new ulong[Tiles*Tiles][];
        private ulong revision=ulong.MaxValue;
        private ulong bedVersion=ulong.MaxValue;
        private ulong coastShapeRevision=ulong.MaxValue;
        private ulong resetRevision=ulong.MaxValue;
        private readonly ulong[] tileRevisions=new ulong[Tiles*Tiles];
        internal int LastRebuiltTileCount { get; private set; }
        internal int LastUpdatedVertexCount { get; private set; }

        internal ModernWaterBed(Transform parent, Material material)
        {
            root=new GameObject("Visual lake bed"); root.transform.SetParent(parent,false);
            for(var i=0;i<meshes.Length;i++)
            {
                meshes[i]=new Mesh { name="Visual lake bed tile "+i };
                nodes[i]=new GameObject("Lake bed "+i); nodes[i].transform.SetParent(root.transform,false);
                nodes[i].AddComponent<MeshFilter>().sharedMesh=meshes[i];
                var renderer=nodes[i].AddComponent<MeshRenderer>(); renderer.sharedMaterial=material;
                renderer.shadowCastingMode=ShadowCastingMode.Off;
                renderer.lightProbeUsage=LightProbeUsage.Off; renderer.reflectionProbeUsage=ReflectionProbeUsage.Off;
                nodes[i].SetActive(false);
            }
        }

        internal static float Height(ModernTerrainMesh terrain, ModernWaterCoast coast, float x, float z)
        {
            var surface=terrain.SampleHeight(x,z);
            // Bed vertices on a dry/wet boundary must meet the land exactly.
            // Even a small negative bias exposes the sky through a shoreline slit
            // once the water becomes transparent.
            if(!terrain.IsWater(x-.01f,z-.01f)||!terrain.IsWater(x+.01f,z-.01f)
                ||!terrain.IsWater(x-.01f,z+.01f)||!terrain.IsWater(x+.01f,z+.01f)) return surface;
            var distance=coast.SampleDistance(x,z);
            var depth=ModernWaterCoast.VisualDepth(distance)*Mathf.SmoothStep(0,1,Mathf.Clamp01(distance/6));
            return surface-depth;
        }

        internal void Update(ModernTerrainMesh terrain, ModernWaterCoast coast, Vector3 camera)
        {
            LastRebuiltTileCount = 0;
            LastUpdatedVertexCount = 0;
            if(revision!=terrain.Revision || bedVersion!=terrain.WaterBedVersion)
            {
                if(coast.Revision!=terrain.Revision) throw new InvalidOperationException("Lake bed requires matching coast and terrain revisions");
                bool all = revision == ulong.MaxValue || coastShapeRevision != coast.ShapeRevision
                    || resetRevision != terrain.WaterBedResetRevision;
                for(var z=0;z<Tiles;z++) for(var x=0;x<Tiles;x++)
                {
                    int i = x + z * Tiles;
                    ulong next = terrain.WaterBedRevision(x, z);
                    if (!all && tileRevisions[i] == next) continue;
                    if (all) Rebuild(x,z,terrain,coast);
                    else UpdateVertices(x,z,terrain,coast);
                    tileRevisions[i] = next; LastRebuiltTileCount++;
                }
                coastShapeRevision = coast.ShapeRevision;
                resetRevision = terrain.WaterBedResetRevision;
                revision=terrain.Revision;
                bedVersion=terrain.WaterBedVersion;
                if (ModernCapture.Measure) Debug.Log($"MC_MODERN_WATERBED revision={revision} coast_shape={coastShapeRevision} rebuilt_tiles={LastRebuiltTileCount} updated_vertices={LastUpdatedVertexCount}");
            }
            for(var z=0;z<Tiles;z++) for(var x=0;x<Tiles;x++)
            {
                var i=x+z*Tiles;
                var px=x*128+Mathf.Round((camera.x-x*128-64)/1024)*1024;
                var pz=z*128+Mathf.Round((camera.z-z*128-64)/1024)*1024;
                nodes[i].transform.position=new Vector3(px,0,pz);
                var dx=camera.x-Mathf.Clamp(camera.x,px,px+128);
                var dz=camera.z-Mathf.Clamp(camera.z,pz,pz+128);
                // Beyond this the surface is opaque at ordinary viewing angles.
                var visible=populated[i] && dx*dx+dz*dz<320*320;
                if(nodes[i].activeSelf!=visible) nodes[i].SetActive(visible);
            }
        }

        private void Rebuild(int tx, int tz, ModernTerrainMesh terrain, ModernWaterCoast coast)
        {
            var i=tx+tz*Tiles; var mesh=meshes[i];
            var indices=new List<int>();
            for(var z=0;z<Cells;z++) for(var x=0;x<Cells;x++)
            {
                var wx=tx*128+x*2+1;var wz=tz*128+z*2+1;
                if(!terrain.IsWater(wx,wz)) continue;
                var a=x+z*Side; var b=a+1; var c=a+Side; var d=c+1;
                if(((x/2+z/2)&1)==0) { indices.Add(a);indices.Add(c);indices.Add(d);indices.Add(a);indices.Add(d);indices.Add(b); }
                else { indices.Add(a);indices.Add(c);indices.Add(b);indices.Add(c);indices.Add(d);indices.Add(b); }
            }
            mesh.Clear(); populated[i]=indices.Count>0;
            if(!populated[i])
            {
                verticesByTile[i]=null; normalsByTile[i]=null; colorsByTile[i]=null; vertexRevisions[i]=null;
                return;
            }
            var vertices=verticesByTile[i]=new Vector3[Side*Side];
            var normals=normalsByTile[i]=new Vector3[vertices.Length];
            var colors=colorsByTile[i]=new Color[vertices.Length];
            var revisions=vertexRevisions[i]=new ulong[vertices.Length];
            for(var z=0;z<Side;z++) for(var x=0;x<Side;x++)
            {
                var wx=tx*128+x*2;var wz=tz*128+z*2;var v=x+z*Side;
                vertices[v]=new Vector3(x*2,Height(terrain,coast,wx,wz),z*2);
                normals[v]=new Vector3(Height(terrain,coast,wx-1,wz)-Height(terrain,coast,wx+1,wz),2,
                    Height(terrain,coast,wx,wz-1)-Height(terrain,coast,wx,wz+1)).normalized;
                colors[v]=terrain.DetailColor(wx/2,wz/2);
                revisions[v]=terrain.WaterBedVertexRevision(wx/2,wz/2);
            }
            LastUpdatedVertexCount += vertices.Length;
            mesh.vertices=vertices;mesh.normals=normals;mesh.colors=colors;mesh.SetTriangles(indices,0);mesh.RecalculateBounds();
        }

        private void UpdateVertices(int tx, int tz, ModernTerrainMesh terrain, ModernWaterCoast coast)
        {
            var i=tx+tz*Tiles;
            if(!populated[i]) return; // An unchanged coast has unchanged topology.
            var vertices=verticesByTile[i]; var normals=normalsByTile[i];
            var colors=colorsByTile[i]; var revisions=vertexRevisions[i];
            var updated=0;
            for(var z=0;z<Side;z++) for(var x=0;x<Side;x++)
            {
                var wx=tx*128+x*2; var wz=tz*128+z*2; var v=x+z*Side;
                var next=terrain.WaterBedVertexRevision(wx/2,wz/2);
                if(revisions[v]==next) continue;
                vertices[v]=new Vector3(x*2,Height(terrain,coast,wx,wz),z*2);
                normals[v]=new Vector3(Height(terrain,coast,wx-1,wz)-Height(terrain,coast,wx+1,wz),2,
                    Height(terrain,coast,wx,wz-1)-Height(terrain,coast,wx,wz+1)).normalized;
                colors[v]=terrain.DetailColor(wx/2,wz/2);
                revisions[v]=next; updated++;
            }
            LastUpdatedVertexCount+=updated;
            if(updated==0) return;
            var mesh=meshes[i];
            mesh.vertices=vertices;mesh.normals=normals;mesh.colors=colors;mesh.RecalculateBounds();
        }

        public void Dispose()
        {
            foreach(var mesh in meshes) Destroy(mesh);
            Destroy(root);
        }

        private static void Destroy(UnityEngine.Object item)
        {
            if(Application.isPlaying) UnityEngine.Object.Destroy(item);
            else UnityEngine.Object.DestroyImmediate(item);
        }
    }
}
