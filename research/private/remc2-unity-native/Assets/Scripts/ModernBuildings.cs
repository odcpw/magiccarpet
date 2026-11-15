using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace MagicCarpet.Native
{
    // Source-layout presentation. Rust owns the height stamp and gameplay;
    // live substitutions require the current source owner and exact cells.
    internal sealed class ModernBuildings : IDisposable
    {
        private sealed class Instance
        {
            internal GameObject Node;
            internal Vector3 Position;
            internal Vector2 Size;
            internal int Slot;
            internal Vector3[] WindowCenters, WindowNormals;
            internal Foundation Foundation;
        }
        internal struct LiveBuilding
        {
            internal int Slot, BuildingIndex, Action;
            internal uint SlotGeneration;
            internal bool Drawing;
            internal ushort X, Y;
            internal short Z;
        }
        private readonly Transform parent;
        private readonly List<Instance> instances=new();
        private readonly Dictionary<int,Mesh[]> meshes=new();
        private readonly Material material;
        private readonly ModernBuildingLayout layout;
        private readonly bool voxel;
        private readonly ModernTerrainMesh terrain;
        private readonly List<Vector3> windowCenters=new(),windowNormals=new();
        private readonly List<Bounds> changedFootprints=new();
        private bool prepared;
        private HashSet<int> liveFlattened, liveVisible;
        internal Vector3[] WindowCenters => windowCenters.ToArray();
        internal Vector3[] WindowNormals => windowNormals.ToArray();
        internal IReadOnlyList<Bounds> ChangedFootprints => changedFootprints;
        internal Vector3 Center { get; private set; }
        internal Vector3 House { get; private set; }
        internal Vector3 Hall { get; private set; }
        internal Vector3 Tower { get; private set; }
        internal Vector3 Coastal { get; private set; }
        internal int Count => instances.Count;
        internal int VisibleCount => liveVisible?.Count ?? instances.Count;
        internal bool Ready => Count>0;

        internal ModernBuildings(Transform parent,ModernTerrainMesh terrain,ModernBuildingLayout source=null,bool voxel=false)
        {
            this.parent=parent;
            this.voxel=voxel;
            this.terrain=terrain;
            layout=source??ModernBuildingLayout.LoadRequested();
            if(layout==null) return;
            layout.ValidateProfiles();
            var shader=Resources.Load<Shader>("ModernBuilding");
            if(shader==null) throw new InvalidOperationException("Missing ModernBuilding shader");
            material=new Material(shader) { name="NightSpire source architecture",enableInstancing=true };
            material.SetTexture("_StoneMap",Resources.Load<Texture2D>("ModernTextures/RockAlbedoRoughness"));
            try
            {
                foreach(var profile in layout.profiles)
                {
                    var levels=new Mesh[3];meshes.Add(profile.build_index,levels);
                    for(var lod=0;lod<3;lod++) levels[lod]=voxel
                        ? ModernBuildingMeshes.CreateVoxel(profile,lod) : ModernBuildingMeshes.Create(profile,lod);
                }
            }
            catch { Dispose();throw; }
        }

        internal static float WrappedDelta(float a,float b) => Mathf.Repeat(a-b+512,1024)-512;
        internal byte[] PrepareTerrain(byte[] original)
        {
            if(layout==null) return original;
            var result=layout.VisualHeights(original);prepared=true;return result;
        }

        // Every stamp is an atomic substitution. An unrelated terrain change
        // may coexist with a valid stamp; an altered stamp is never flattened.
        // The caller retains the untouched native height plane for the next
        // reconciliation and rebuilds terrain when `changed` is true.
        internal byte[] Reconcile(byte[] original, IReadOnlyList<LiveBuilding> live, int voxelSlot, out bool changed)
        {
            if(layout==null) throw new InvalidOperationException("Live buildings require a source layout");
            if(live==null) throw new ArgumentNullException(nameof(live));
            layout.ValidateCells(original,false);
            var bySlot=new Dictionary<int,LiveBuilding>();
            foreach(var row in live)
                if(!bySlot.TryAdd(row.Slot,row)) throw new InvalidOperationException("Duplicate live building slot");
            var flattened=new HashSet<int>();var visible=new HashSet<int>();
            foreach(var entry in layout.buildings)
            {
                var source=entry.source_entity_position;
                if(source==null||source.x<0||source.x>ushort.MaxValue||source.y<0||source.y>ushort.MaxValue||
                    source.z<short.MinValue||source.z>short.MaxValue)
                    throw new System.IO.InvalidDataException("Building source position is missing or invalid");
                if(!bySlot.TryGetValue(entry.source_slot,out var row)||row.SlotGeneration!=0||
                    row.BuildingIndex!=entry.build_index||row.Action!=52||!row.Drawing||row.X!=source.x||row.Y!=source.y||row.Z!=source.z)
                    continue;
                var intact=true;
                foreach(var cell in entry.cells) if(original[cell.index]!=cell.expected) { intact=false;break; }
                if(!intact) continue;
                flattened.Add(entry.source_slot);
                if(entry.source_slot!=voxelSlot) visible.Add(entry.source_slot);
            }
            changed=liveFlattened==null||!liveFlattened.SetEquals(flattened)||!liveVisible.SetEquals(visible);
            changedFootprints.Clear();
            foreach(var entry in layout.buildings)
                if((liveVisible?.Contains(entry.source_slot)??false)!=visible.Contains(entry.source_slot))
                    changedFootprints.Add(new Bounds(new Vector3(entry.x,entry.base_height,entry.z),
                        new Vector3(entry.width+6,200,entry.depth+6)));
            var visual=(byte[])original.Clone();
            foreach(var entry in layout.buildings)
                if(flattened.Contains(entry.source_slot))
                    foreach(var cell in entry.cells) visual[cell.index]=(byte)cell.replacement;
            prepared=true;
            if(!Ready) Stage(Vector3.zero);
            foreach(var instance in instances) instance.Node.SetActive(visible.Contains(instance.Slot));
            if(liveVisible==null||!liveVisible.SetEquals(visible)) RebuildWindows();
            liveFlattened=flattened;liveVisible=visible;
            return visual;
        }
        internal bool ExcludesVegetation(float x,float z)
        {
            foreach(var instance in instances)
                if(instance.Node.activeSelf&&Mathf.Abs(WrappedDelta(x,instance.Position.x))<instance.Size.x*.5f+3
                    &&Mathf.Abs(WrappedDelta(z,instance.Position.z))<instance.Size.y*.5f+3) return true;
            return false;
        }

        internal void Stage(Vector3 preferred)
        {
            if(Ready||layout==null) return;
            if(!prepared) throw new InvalidOperationException("Building terrain must validate before staging");
            try
            {
                foreach(var entry in layout.buildings) Create(entry);
                Hall=Closest(29,preferred);
                House=Closest(26,Hall);
                Tower=Closest(42,Hall);
                Center=(Hall+House)*.5f;
                Coastal=Hall;
                foreach(var entry in layout.buildings)
                    if(entry.base_height<.1f&&entry.build_index==29)
                    { Coastal=new Vector3(entry.x,.025f,entry.z);break; }
                Debug.Log($"MC_BUILDINGS_STAGED count={Count} hall={Hall} house={House} tower={Tower} level1_source_positions=True source_profiles=True gameplay_binding=False");
            }
            catch { ClearInstances();throw; }
        }

        private Vector3 Closest(int selector,Vector3 target)
        {
            var best=target;var score=float.PositiveInfinity;
            foreach(var entry in layout.buildings)
            {
                if(entry.build_index!=selector) continue;
                var dx=WrappedDelta(entry.x,target.x);var dz=WrappedDelta(entry.z,target.z);var distance=dx*dx+dz*dz;
                if(distance<score) { score=distance;best=new Vector3(entry.x,entry.base_height+.025f,entry.z); }
            }
            return best;
        }

        private void Create(ModernBuildingLayout.Entry entry)
        {
            var profile=layout.Profile(entry.build_index);
            var levels=meshes[entry.build_index];
            var best=new Vector3(entry.x,entry.base_height+.025f,entry.z);
            var node=new GameObject($"Level 1 building {entry.source_slot} BUILD {entry.build_index}");
            node.transform.SetParent(parent,false);node.transform.position=best;
            // Register before creating children so rollback also owns partial instances.
            var instance=new Instance { Node=node,Position=best,Size=ModernBuildingMeshes.Footprint(profile),Slot=entry.source_slot };
            instances.Add(instance);
            if(terrain!=null) instance.Foundation=new Foundation(node.transform,profile,material);
            var lods=new LOD[3];var thresholds=new[] {.22f,.07f,.008f};
            for(var lod=0;lod<3;lod++)
            {
                var child=new GameObject($"BUILD {entry.build_index} detail {lod}");child.transform.SetParent(node.transform,false);
                child.AddComponent<MeshFilter>().sharedMesh=levels[lod];
                var renderer=child.AddComponent<MeshRenderer>();renderer.sharedMaterial=material;
                renderer.shadowCastingMode=ShadowCastingMode.On;renderer.receiveShadows=true;
                lods[lod]=new LOD(thresholds[lod],instance.Foundation==null
                    ? new Renderer[] { renderer } : new Renderer[] { renderer,instance.Foundation.Renderer });
            }
            var group=node.AddComponent<LODGroup>();group.SetLODs(lods);group.RecalculateBounds();
            var centers=voxel ? ModernBuildingMeshes.VoxelWindowCenters(profile,0) : ModernBuildingMeshes.WindowCenters(profile);
            var normals=ModernBuildingMeshes.WindowNormals(profile);
            instance.WindowCenters=new Vector3[centers.Length];instance.WindowNormals=normals;
            for(var i=0;i<centers.Length;i++) { instance.WindowCenters[i]=best+centers[i];windowCenters.Add(instance.WindowCenters[i]);windowNormals.Add(normals[i]); }
        }

        private void RebuildWindows()
        {
            windowCenters.Clear();windowNormals.Clear();
            foreach(var instance in instances)
                if(instance.Node.activeSelf)
                    for(var i=0;i<instance.WindowCenters.Length;i++)
                    { windowCenters.Add(instance.WindowCenters[i]);windowNormals.Add(instance.WindowNormals[i]); }
        }

        internal void Render(Vector3 viewer)
        {
            foreach(var item in instances)
            {
                item.Node.transform.position=new Vector3(viewer.x+WrappedDelta(item.Position.x,viewer.x),
                    item.Position.y,viewer.z+WrappedDelta(item.Position.z,viewer.z));
                if(item.Node.activeSelf&&item.Foundation!=null &&
                    item.Foundation.Refresh(item.Node.transform.position,terrain.SampleVisualHeight))
                    item.Node.GetComponent<LODGroup>().RecalculateBounds();
            }
        }
        internal void SetLighting(int mode) => material?.SetFloat("_WindowLight",mode==0?0:mode==1?.45f:.8f);

        public void Dispose()
        {
            ClearInstances();
            foreach(var levels in meshes.Values) foreach(var mesh in levels) Destroy(mesh);
            meshes.Clear();Destroy(material);
        }
        private void ClearInstances()
        {
            foreach(var item in instances) { item.Foundation?.Dispose();Destroy(item.Node); }
            instances.Clear();windowCenters.Clear();windowNormals.Clear();
            changedFootprints.Clear();
            liveFlattened=liveVisible=null;
            Center=House=Hall=Tower=Coastal=Vector3.zero;
        }
        private static void Destroy(UnityEngine.Object item)
        { if(item==null)return;if(Application.isPlaying)UnityEngine.Object.Destroy(item);else UnityEngine.Object.DestroyImmediate(item); }

        // One exterior skirt per instance, sampled on the source-profile perimeter.
        // Shared roof meshes and native heights remain untouched. A metre spacing
        // resolves the terrain triangles while keeping the wall bounded by perimeter.
        private sealed class Foundation : IDisposable
        {
            private readonly Vector3[] edge;
            private readonly float[] ground;
            private readonly Mesh mesh;
            internal readonly MeshRenderer Renderer;

            internal Foundation(Transform parent,ModernBuildingMeshes.Profile profile,Material material)
            {
                int nx=(profile.columns-1)*4,nz=(profile.rows-1)*4;
                edge=new Vector3[2*(nx+nz)];ground=new float[edge.Length];
                int n=0;
                void Add(int x,int z)
                {
                    int ix=Mathf.Min(x/4,profile.columns-2),iz=Mathf.Min(z/4,profile.rows-2);
                    float u=x/4f-ix,v=z/4f-iz;
                    // Along an outer edge, bilinear interpolation is identical to
                    // either of the source triangles sharing that edge.
                    float a=profile.heights[ix+iz*profile.columns],b=profile.heights[ix+1+iz*profile.columns];
                    float c=profile.heights[ix+(iz+1)*profile.columns],d=profile.heights[ix+1+(iz+1)*profile.columns];
                    edge[n]=new Vector3(x-nx*.5f,Mathf.Lerp(Mathf.Lerp(a,b,u),Mathf.Lerp(c,d,u),v),z-nz*.5f);
                    ground[n++]=float.NaN;
                }
                for(int x=0;x<nx;x++)Add(x,0);
                for(int z=0;z<nz;z++)Add(nx,z);
                for(int x=nx;x>0;x--)Add(x,nz);
                for(int z=nz;z>0;z--)Add(0,z);
                var child=new GameObject("Source building foundation");child.transform.SetParent(parent,false);
                mesh=new Mesh { name="Source building foundation" };
                child.AddComponent<MeshFilter>().sharedMesh=mesh;
                Renderer=child.AddComponent<MeshRenderer>();Renderer.sharedMaterial=material;
                Renderer.shadowCastingMode=ShadowCastingMode.On;Renderer.receiveShadows=true;
            }

            internal bool Refresh(Vector3 position,Func<float,float,float> sample)
            {
                bool changed=false;
                for(int i=0;i<edge.Length;i++)
                {
                    float y=sample(position.x+edge[i].x,position.z+edge[i].z)-position.y;
                    if(float.IsNaN(y)||float.IsInfinity(y)) throw new InvalidOperationException("Invalid building foundation terrain height");
                    if(float.IsNaN(ground[i])||Mathf.Abs(ground[i]-y)>.001f) { ground[i]=y;changed=true; }
                }
                if(!changed)return false;
                var vertices=new List<Vector3>();var normals=new List<Vector3>();var uv=new List<Vector2>();
                var palette=new List<Vector2>();var colors=new List<Color>();var triangles=new List<int>();
                for(int i=0;i<edge.Length;i++)
                {
                    int j=(i+1)%edge.Length;
                    // Ignore the existing 0.025 placement offset on flat ground.
                    float d0=edge[i].y-ground[i],d1=edge[j].y-ground[j];
                    if(d0<=.05f&&d1<=.05f)continue;
                    Vector3 a=edge[i],b=edge[j],c=b,d=a;
                    c.y=Mathf.Min(b.y,ground[j]-.025f);d.y=Mathf.Min(a.y,ground[i]-.025f);
                    var normal=new Vector3(b.z-a.z,0,a.x-b.x).normalized;
                    int first=vertices.Count;
                    foreach(var point in new[] { a,b,c,d })
                    { vertices.Add(point);normals.Add(normal);uv.Add(new Vector2(point.x,point.z));palette.Add(Vector2.zero);colors.Add(Color.white); }
                    if((b.y-c.y)>.0001f) { triangles.Add(first);triangles.Add(first+1);triangles.Add(first+2); }
                    if((a.y-d.y)>.0001f) { triangles.Add(first);triangles.Add(first+2);triangles.Add(first+3); }
                }
                mesh.Clear();mesh.SetVertices(vertices);mesh.SetNormals(normals);mesh.SetUVs(0,uv);mesh.SetUVs(1,palette);
                mesh.SetColors(colors);mesh.SetTriangles(triangles,0);mesh.RecalculateBounds();
                Renderer.enabled=triangles.Count!=0;
                return true;
            }

            public void Dispose() => Destroy(mesh);
        }
    }
}
