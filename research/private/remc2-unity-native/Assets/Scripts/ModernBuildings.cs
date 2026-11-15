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
            var lods=new LOD[3];var thresholds=new[] {.22f,.07f,.008f};
            for(var lod=0;lod<3;lod++)
            {
                var child=new GameObject($"BUILD {entry.build_index} detail {lod}");child.transform.SetParent(node.transform,false);
                child.AddComponent<MeshFilter>().sharedMesh=levels[lod];
                var renderer=child.AddComponent<MeshRenderer>();renderer.sharedMaterial=material;
                renderer.shadowCastingMode=ShadowCastingMode.On;renderer.receiveShadows=true;
                lods[lod]=new LOD(thresholds[lod],new Renderer[] { renderer });
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
                item.Node.transform.position=new Vector3(viewer.x+WrappedDelta(item.Position.x,viewer.x),
                    item.Position.y,viewer.z+WrappedDelta(item.Position.z,viewer.z));
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
            foreach(var item in instances) Destroy(item.Node);
            instances.Clear();windowCenters.Clear();windowNormals.Clear();
            changedFootprints.Clear();
            liveFlattened=liveVisible=null;
            Center=House=Hall=Tower=Coastal=Vector3.zero;
        }
        private static void Destroy(UnityEngine.Object item)
        { if(item==null)return;if(Application.isPlaying)UnityEngine.Object.Destroy(item);else UnityEngine.Object.DestroyImmediate(item); }
    }
}
