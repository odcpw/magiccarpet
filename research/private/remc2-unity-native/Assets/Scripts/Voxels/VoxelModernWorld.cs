using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using UnityEngine;
using Debug = UnityEngine.Debug;

namespace MagicCarpet.Native.Voxels
{
    // Opt-in fine-voxel presentation of the native campaign world.
    // Native state owns occupancy, impacts, gameplay and persistence.
    internal sealed class VoxelModernWorld : MonoBehaviour
    {
        internal static readonly bool Requested=Environment.GetCommandLineArgs().Contains("-mc-voxel-modern");
        internal static VoxelModernWorld Active { get; private set; }
        internal static readonly bool ReceiptLogging=Array.IndexOf(Environment.GetCommandLineArgs(),"-mc-voxel-receipts")>=0;
        internal const float GroundCell=.125f, BrickCell=.25f;
        private ModernTerrainMesh terrain;
        private ModernVegetation vegetation;
        private readonly List<Vector3> clearedPlants=new();
        private readonly HashSet<Vector2Int> clearedPlantTiles=new();
        private Camera view;
        private ModernEnvironmentSandbox lighting;
        private VoxelSandbox.Inputs input;
        private VoxelVolume ground,house;
        private VoxelTerrainContact contact;
        private VoxelTerrainSeam seam;
        private VoxelRubble rubble;
        private Material surface,groundMaterial;
        private GameObject physical;
        private Bounds patch;
        private Vector3 home;
        private bool ready,editing;
        private float started;
        private int impacts,deaths,removed;
        private int groundRemoved,houseRemoved,completedEdits;
        private float craterDepth;
        private int lightingPreset;
        internal int LightingMode => lightingPreset;
        internal int SelectedBuildingSlot => input?.building?.source_slot ?? -1;
        private string output;
        private string lastBaseHash;
        private bool capture,automatic,nativeCamera;
        private int visualRemoved,visualGroundRemoved,visualHouseRemoved;
        private bool freeFlight,flightPositioned;
        private bool characterReview;
        private float duration=28;
        private ulong firstNativeTick;
        private Vector3 flightPosition;
        private float yaw,pitch;
        private long wall;
        private readonly List<double> frames=new();
        private double maxEditMs;
        private double maxPhysicsMs;
        private double warmPhysicsMaxMs,physicsTotalMs;
        private int physicsTurns;
        private float startedWall;
        private readonly List<double> physicsSamples=new();
        private static SimulationMode originalSimulation;
        private static int physicsOwners,originalSolver,originalVelocitySolver;
        private bool ownsPhysics;
        private ulong nativeTick, worldGeneration = ulong.MaxValue, worldEpoch, lastRevision, lastContentHash, impactCursor;
        private NativeCampaignSession.VoxelVolumeProjection terrainBase, houseBase;
        private int[] topColumns;
        private readonly HashSet<(int kind,int slot,int x,int y,int z)> knownRemoved = new();
        private readonly Queue<(List<Vector3Int> ground,List<Vector3Int> house)> pendingEdits = new();
        private Coroutine editRoutine;

        internal static string Option(string key) { var a=Environment.GetCommandLineArgs();int i=Array.IndexOf(a,key);return i<0?null:i+1<a.Length?a[i+1]:throw new ArgumentException(key+" needs a value"); }

        internal void Configure(ModernTerrainMesh terrain,ModernVegetation vegetation,Camera camera,Material land,ModernWaterSystem water,ModernNightLighting night,Light sun,Material sky)
        {
            Active=this; this.terrain=terrain;this.vegetation=vegetation;view=camera;
            groundMaterial=new Material(land){name="Modern materials on fine voxel terrain"};
            view.cullingMask|=1<<8;
            input=JsonUtility.FromJson<VoxelSandbox.Inputs>(Resources.Load<TextAsset>("ModernCharacters/VoxelSandbox/Level1").text);
            home=new Vector3(input.building.x,input.building.base_height,input.building.z);
            patch=new Bounds(home,new Vector3(32,200,40));
            terrain.VoxelPatch=patch;
            lighting=new ModernEnvironmentSandbox(camera,terrain,water,night,sun,sky);
            lighting.SetLighting(lightingPreset=2); water.SetSandboxWind(1);
            output=Option("-mc-voxel-capture");capture=output!=null;
            output ??= Option("-mc-voxel-benchmark");automatic=output!=null;
            characterReview=Option("-mc-character-follow")!=null;
            nativeCamera=Array.IndexOf(Environment.GetCommandLineArgs(),"-mc-voxel-native-camera")>=0;
            if(Option("-mc-voxel-seconds") is string seconds)duration=float.Parse(seconds,CultureInfo.InvariantCulture);
            if(!float.IsFinite(duration)||duration<4||duration>60)throw new ArgumentException("Review duration must be 4..60 seconds");
            freeFlight=!automatic&&!nativeCamera&&Option("-mc-character-follow")==null;
            if(automatic) {
                if(Directory.Exists(output))throw new IOException("Evidence output must be new");
                Directory.CreateDirectory(output);
                if(capture){Directory.CreateDirectory(Path.Combine(output,"frames"));Time.captureFramerate=30;}
            }
            QualitySettings.vSyncCount=0;Application.targetFrameRate=-1;
        }
        private void Start()
        {
            // Keep native gameplay at its original 30Hz; only local PhysX gets
            // four small collision steps per authoritative turn.
            if(physicsOwners++==0){originalSimulation=Physics.simulationMode;originalSolver=Physics.defaultSolverIterations;originalVelocitySolver=Physics.defaultSolverVelocityIterations;}
            Physics.simulationMode=SimulationMode.Script;ownsPhysics=true;
            Physics.defaultSolverIterations=8;Physics.defaultSolverVelocityIterations=2;
            Application.targetFrameRate=-1;
        }
        private void FixedUpdate()
        {
            if(!ready)return;var begin=Stopwatch.GetTimestamp();
            int steps=Mathf.Clamp(Mathf.CeilToInt(Time.fixedDeltaTime*120),1,8);
            for(int i=0;i<steps;i++)Physics.Simulate(Time.fixedDeltaTime/steps);
            double ms=(Stopwatch.GetTimestamp()-begin)*1000.0/Stopwatch.Frequency;
            maxPhysicsMs=Math.Max(maxPhysicsMs,ms);
            if(Time.time-started>1){warmPhysicsMaxMs=Math.Max(warmPhysicsMaxMs,ms);physicsTotalMs+=ms;physicsTurns++;physicsSamples.Add(ms);}
            if(ms>16)Debug.Log($"MC_VOXEL_PHYSICS_SPIKE t={Time.time-started:F2} ms={ms:F2} bodies={rubble.ActiveBodies} sleeping={rubble.SleepingBodies} pending={rubble.PendingEmissions}");
        }
        internal bool ExcludesVegetation(float x,float z)
        {
            if(Mathf.Abs(x-home.x)<11&&Mathf.Abs(z-home.z)<15)return true;
            foreach(var p in clearedPlants)if((x-p.x)*(x-p.x)+(z-p.z)*(z-p.z)<3.5f*3.5f)return true;
            return false;
        }
        // Capture the unmodified Modern sampler before installing the native override.
        internal static float[] SampleSeedHeights(Bounds patch, Func<float,float,float> sample)
        {
            var result = new float[17*21 + 256*320]; int n=0;
            for(int z=0;z<21;z++) for(int x=0;x<17;x++) result[n++]=sample(patch.min.x+x*2,patch.min.z+z*2);
            for(int z=0;z<320;z++) for(int x=0;x<256;x++) result[n++]=sample(patch.min.x+(x+.5f)*GroundCell,patch.min.z+(z+.5f)*GroundCell);
            return result;
        }
        private static (int,int,int,int,int) CellKey(NativeCampaignSession.VoxelRemovedCell cell) => (cell.kind,cell.slot,cell.x,cell.y,cell.z);
        private static Vector3 Origin(NativeCampaignSession.VoxelVolumeProjection volume) => new Vector3(volume.origin_x,volume.origin_z,volume.origin_y)/64f;
        private static Vector3Int Dimensions(NativeCampaignSession.VoxelVolumeProjection volume) => new Vector3Int(volume.dimensions[0],volume.dimensions[2],volume.dimensions[1]);
        internal static bool BaseBuildingCell(NativeCampaignSession.VoxelVolumeProjection volume,int x,int y,int z)
        {
            int index=checked((z*volume.dimensions[1]+y)*volume.dimensions[0]+x);
            return (volume.building_bytes[index/8] & (1 << (index%8))) != 0;
        }
        private void ResetProjection(bool clearEffects=true)
        {
            if(editRoutine!=null){StopCoroutine(editRoutine);editRoutine=null;}
            editing=false;pendingEdits.Clear();knownRemoved.Clear();visualRemoved=visualGroundRemoved=visualHouseRemoved=0;
            if(clearedPlants.Count>0){clearedPlants.Clear();clearedPlantTiles.Clear();vegetation?.InvalidateFootprint(patch);}
            if(terrain!=null)terrain.VoxelHeightSampler=null;
            if(clearEffects)rubble?.ClearEmissions();
            ground?.Dispose();house?.Dispose();contact?.Dispose();seam?.Dispose();
            ground=null;house=null;contact=null;seam=null;terrainBase=null;houseBase=null;topColumns=null;ready=false;
        }
        internal static bool NeedsRebuild(NativeSim.VoxelState state,ulong tick,ulong previousEpoch,ulong previousTick,ulong revision,ulong contentHash,ulong impactHead)
            => state.worldGeneration!=previousEpoch || tick<previousTick || state.enabled==0 || state.revision<revision || state.impactHead<impactHead || state.revision==revision && state.contentHash!=contentHash;
        internal void Observe(NativeCampaignSession session)
        {
            var state=session.VoxelWorldState();
            bool outerChanged=ready && worldGeneration!=session.Generation;
            if(ready && NeedsRebuild(state,session.Tick,worldEpoch,nativeTick,lastRevision,lastContentHash,impactCursor)) ResetProjection();
            nativeTick=session.Tick;worldEpoch=state.worldGeneration;worldGeneration=session.Generation;
            if(!ready)
            {
                terrain.VoxelHeightSampler=null;
                if(state.enabled==0){session.EnableVoxelWorld(checked((ushort)input.building.source_slot),SampleSeedHeights(patch,terrain.SampleHeight));state=session.VoxelWorldState();worldEpoch=state.worldGeneration;}
                var projection=session.VoxelWorldProjection(true,state.impactHead);
                InitializeProjection(projection);
                if((uint)knownRemoved.Count!=state.removedCount)throw new InvalidOperationException("Native voxel removal count mismatch");
                worldGeneration=session.Generation;lastRevision=state.revision;lastContentHash=state.contentHash;impactCursor=state.impactHead;
                ready=true;LogProjectionState(true);started=Time.time;startedWall=Time.realtimeSinceStartup;firstNativeTick=nativeTick;wall=Stopwatch.GetTimestamp();
                ModernCapture.BeginWorldMeasurement();
                if(capture)StartCoroutine(Record());
                Debug.Log($"MC_VOXEL_MODERN_READY native_tick={nativeTick} revision={lastRevision} removed={knownRemoved.Count} impact_head={impactCursor} native_authority=True");
                return;
            }
            if(state.revision==lastRevision && state.impactHead==impactCursor){if(outerChanged)LogProjectionState(false);return;}
            var next=session.VoxelWorldProjection(false,impactCursor);
            if(!String.Equals(next.base_hash,lastBaseHash,StringComparison.Ordinal))
            {
                var refreshed=session.VoxelWorldProjection(true,state.impactHead);
                if(!String.Equals(refreshed.base_hash,next.base_hash,StringComparison.Ordinal))
                    throw new InvalidOperationException("Native terrain base changed without a simulation step");
                ResetProjection(clearEffects:false);
                InitializeProjection(refreshed);
                ready=true;
            }
            else
            {
                var groundCells=new List<Vector3Int>();var houseCells=new List<Vector3Int>();
                foreach(var cell in next.removed)
                {
                    if(!knownRemoved.Add(CellKey(cell)))continue;
                    if(cell.kind==0 && cell.slot==terrainBase.slot){groundCells.Add(new Vector3Int(cell.x,cell.z,cell.y));UpdateTop(cell.x,cell.y);ClearGroundPlants(cell.x,cell.y);}
                    else if(cell.kind==1 && cell.slot==houseBase.slot)houseCells.Add(new Vector3Int(cell.x,cell.z,cell.y));
                    else throw new InvalidOperationException("Unknown native voxel volume");
                }
                if(groundCells.Count!=0||houseCells.Count!=0){pendingEdits.Enqueue((groundCells,houseCells));if(editRoutine==null)editRoutine=StartCoroutine(ApplyNativeEdits());}
            }
            if((uint)knownRemoved.Count!=state.removedCount)throw new InvalidOperationException("Native voxel removal count mismatch");
            foreach(var impact in next.impacts)
            {
                if(!ulong.TryParse(impact.sequence,out var sequence) || sequence!=impactCursor+1)throw new InvalidOperationException("Native impact ledger has a gap");
                EmitNativeImpact(impact);impactCursor=sequence;
            }
            if(impactCursor!=state.impactHead)throw new InvalidOperationException("Native impact ledger has a gap");
            lastRevision=state.revision;lastContentHash=state.contentHash;
            LogProjectionState(false);
        }
        private void LogProjectionState(bool restored)
        {
            if(!ReceiptLogging)return;
            var nativeGround=knownRemoved.Count(cell=>cell.kind==0);
            Debug.Log($"MC_VOXEL_RECEIPT kind=state generation={worldGeneration} tick={nativeTick} revision={lastRevision} content_hash={lastContentHash} native_removed={knownRemoved.Count} visual_removed={visualRemoved} pending={(editing||pendingEdits.Count!=0?1:0)} restored={(restored?1:0)} world_epoch={worldEpoch} house_slot={houseBase.slot} native_ground={nativeGround} visual_ground={visualGroundRemoved} native_house={knownRemoved.Count-nativeGround} visual_house={visualHouseRemoved}");
        }
        private void InitializeProjection(NativeCampaignSession.VoxelProjection projection)
        {
            lastBaseHash=projection.base_hash;
            terrainBase=projection.volumes.Single(v=>v.kind==0);
            houseBase=projection.volumes.Single(v=>v.kind==1 && v.slot==input.building.source_slot);
            if(terrainBase.dimensions==null||terrainBase.dimensions.Length!=3||terrainBase.terrain_columns==null||terrainBase.terrain_columns.Length!=terrainBase.dimensions[0]*terrainBase.dimensions[1] || houseBase.dimensions==null||houseBase.dimensions.Length!=3||houseBase.building_bytes==null)
                throw new InvalidOperationException("Invalid native voxel base projection");
            int currentlyRemoved=0;
            foreach(var cell in projection.removed)
            {
                if(cell.kind==0 && cell.slot!=terrainBase.slot || cell.kind==1 && cell.slot!=houseBase.slot || cell.kind<0 || cell.kind>1 || !knownRemoved.Add(CellKey(cell)))
                    throw new InvalidOperationException("Invalid native removed cell identity");
                if(RemovedCellIntersectsBase(cell.kind==0?terrainBase:houseBase,cell))currentlyRemoved++;
            }
            topColumns=(int[])terrainBase.terrain_columns.Clone();
            foreach(var cell in projection.removed)if(cell.kind==0 && cell.slot==terrainBase.slot){UpdateTop(cell.x,cell.y);ClearGroundPlants(cell.x,cell.y);}
            if(surface==null)surface=new Material(Shader.Find("MagicCarpet/VoxelSurface"));
            if(physical==null){physical=new GameObject("Native fine voxel presentation");physical.layer=8;physical.transform.SetParent(transform,false);rubble=physical.AddComponent<VoxelRubble>();}
            ground=CreateVolume(terrainBase,groundMaterial);house=CreateVolume(houseBase,surface);
            if(visualRemoved!=currentlyRemoved)throw new InvalidOperationException("Restored removal did not match current native base occupancy");
            terrain.VoxelHeightSampler=NativeHeight;
            contact=new VoxelTerrainContact(physical.transform,terrain.SampleHeight);contact.Update(home,patch);
            seam=new VoxelTerrainSeam(physical.transform,patch,terrain.SampleHeight,groundMaterial);
            Physics.SyncTransforms();
        }
        // Source terrain may fall below a previously carved cell. Keep its
        // removal intent, but count only cells omitted from the current base.
        internal static bool RemovedCellIntersectsBase(
            NativeCampaignSession.VoxelVolumeProjection volume,NativeCampaignSession.VoxelRemovedCell cell)
        {
            var dimensions=volume?.dimensions;
            if(cell==null||dimensions==null||dimensions.Length!=3||cell.kind!=volume.kind||cell.slot!=volume.slot
                ||cell.x<0||cell.y<0||cell.z<0||cell.x>=dimensions[0]||cell.y>=dimensions[1]||cell.z>=dimensions[2])
                throw new InvalidOperationException("Native removed cell is outside its current volume");
            if(volume.kind==0)
                return cell.z<volume.terrain_columns[checked(cell.y*dimensions[0]+cell.x)];
            if(volume.kind==1)return BaseBuildingCell(volume,cell.x,cell.y,cell.z);
            throw new InvalidOperationException("Unknown native voxel volume");
        }

        private VoxelVolume CreateVolume(NativeCampaignSession.VoxelVolumeProjection data,Material material)
        {
            var origin=Origin(data);var size=data.cell_size/64f;var dims=Dimensions(data);
            var groupOption=Option("-mc-voxel-render-group");
            int renderGroup=groupOption==null?4:int.Parse(groupOption,CultureInfo.InvariantCulture);
            var volume=new VoxelVolume(physical.transform,origin,dims,size,material,renderGroup);
            for(int z=0;z<dims.z;z++)for(int x=0;x<dims.x;x++)
            {
                int count=data.kind==0?data.terrain_columns[z*dims.x+x]:dims.y;
                float wx=origin.x+(x+.5f)*size,wz=origin.z+(z+.5f)*size;
                float noise=Mathf.PerlinNoise(wx*.39f,wz*.39f);
                bool roof=data.kind==1 && HouseAppearanceTop(input.profile,(x+.5f)*size,(z+.5f)*size)>11.6f;
                for(int y=0;y<count;y++)
                {
                    if(data.kind==1&&!BaseBuildingCell(data,x,z,y))continue;
                    if(knownRemoved.Contains((data.kind,data.slot,x,z,y))){visualRemoved++;if(data.kind==0)visualGroundRemoved++;else visualHouseRemoved++;continue;}
                    Color color;
                    if(data.kind==0)color=count-y>3?Color.Lerp(new Color(.30f,.25f,.18f),new Color(.48f,.39f,.28f),noise):Color.Lerp(new Color(.43f,.41f,.30f),new Color(.64f,.60f,.44f),noise);
                    else {float grain=Mathf.PerlinNoise((x+.5f)*size*5+4,(z+.5f)*size*5+y*.29f);color=roof?Color.Lerp(new Color(.19f,.24f,.27f),new Color(.32f,.37f,.37f),grain):Color.Lerp(new Color(.53f,.46f,.33f),new Color(.75f,.67f,.49f),grain);}
                    volume.SetCell(x,y,z,color);
                }
            }
            volume.Build();return volume;
        }
        // The owner-package profile is used only for the existing roof colors;
        // native packed occupancy determines every actual building cell.
        internal static float HouseAppearanceTop(ModernBuildingMeshes.Profile p,float x,float z)
        {
            float u=x/4f,v=z/4f;
            int ix=Mathf.Min((int)u,p.columns-2),iz=Mathf.Min((int)v,p.rows-2);u-=ix;v-=iz;
            float a=p.heights[ix+iz*p.columns],b=p.heights[ix+1+iz*p.columns],c=p.heights[ix+(iz+1)*p.columns],d=p.heights[ix+1+(iz+1)*p.columns];
            return ((ix+iz+p.diagonal_parity)&1)==0 ? (u<v ? a+(d-c)*u+(c-a)*v : a+(b-a)*u+(d-b)*v) : (u+v<1 ? a+(b-a)*u+(c-a)*v : d+(c-d)*(1-u)+(b-d)*(1-v));
        }
        private void UpdateTop(int x,int z)
        {
            int width=terrainBase.dimensions[0], depth=terrainBase.dimensions[1];
            if(x<0||z<0||x>=width||z>=depth)throw new InvalidOperationException("Native terrain cell outside volume");
            int index=z*width+x;
            topColumns[index]=ColumnTop(topColumns[index],terrainBase.slot,x,z,knownRemoved);
        }
        internal static int ColumnTop(int top,int slot,int x,int z,HashSet<(int kind,int slot,int x,int y,int z)> removed)
        {
            while(top>0&&removed.Contains((0,slot,x,z,top-1)))top--;
            return top;
        }
        private void ClearGroundPlants(int x,int z)
        {
            float size=terrainBase.cell_size/64f;var origin=Origin(terrainBase);
            var position=new Vector3(origin.x+(x+.5f)*size,origin.y,origin.z+(z+.5f)*size);
            var tile=new Vector2Int(Mathf.FloorToInt(position.x/4),Mathf.FloorToInt(position.z/4));
            if(!clearedPlantTiles.Add(tile))return;
            position.x=tile.x*4+2;position.z=tile.y*4+2;clearedPlants.Add(position);
            vegetation.InvalidateFootprint(new Bounds(position,Vector3.one*8));
        }
        private float? NativeHeight(float x,float z)
        {
            if(terrainBase==null||topColumns==null)return null;
            var origin=Origin(terrainBase);float size=terrainBase.cell_size/64f;
            int ix=Mathf.FloorToInt((x-origin.x)/size),iz=Mathf.FloorToInt((z-origin.z)/size);
            if(ix<0||iz<0||ix>=terrainBase.dimensions[0]||iz>=terrainBase.dimensions[1])return null;
            return origin.y+topColumns[iz*terrainBase.dimensions[0]+ix]*size;
        }
        private IEnumerator ApplyNativeEdits()
        {
            editing=true;
            while(pendingEdits.Count>0)
            {
                var cells=pendingEdits.Dequeue();
                foreach(var pair in new[]{(ground,cells.ground),(house,cells.house)})
                {
                    if(pair.Item2.Count==0)continue;
                    var edit=pair.Item1.BeginRemoveCells(pair.Item2);
                    while(!edit.Complete){edit.Step(1);yield return null;}
                    removed+=edit.Removed.Count;visualRemoved+=edit.Removed.Count;maxEditMs=Math.Max(maxEditMs,edit.MaximumStepMilliseconds);
                    if(pair.Item1==ground){groundRemoved+=edit.Removed.Count;visualGroundRemoved+=edit.Removed.Count;}
                    else {houseRemoved+=edit.Removed.Count;visualHouseRemoved+=edit.Removed.Count;}
                    if(edit.Removed.Count>0){var position=edit.Removed[0].Position;rubble.Emit(edit.Removed,Vector3.up*.6f,position,120);}
                }
                completedEdits++;
            }
            editing=false;editRoutine=null;LogProjectionState(false);
        }
        // Native velocity is displacement in 1/64 units per 30 Hz tick.
        internal static Vector3 ImpactVelocity(NativeCampaignSession.VoxelImpact impact)
            => new Vector3(impact.velocity_x,impact.velocity_z,impact.velocity_y)*(30f/64f);
        internal static Vector3 ImpactNormal(NativeCampaignSession.VoxelImpact impact)
            => new Vector3(impact.normal_x,impact.normal_z,impact.normal_y);
        internal static void PrepareEmissionContact(VoxelTerrainContact contact,Vector3 camera,Bounds patch,
            IReadOnlyList<Vector3> active,ulong revision,Vector3 point)
        {
            contact.Update(camera,patch,active,revision,point);
            Physics.SyncTransforms();
        }
        internal static bool TryUnknownGroundClearance(VoxelVolume ground,Vector3 from,Vector3 point,
            List<VoxelFragment> fragments,out Vector3 offset)
        {
            offset=Vector3.zero;
            if(!ground.RaycastSurface(from,point,out var hit))
            {
                // The center chunk may already be committed while an adjacent
                // chunk under the same physical sphere is still pending.
                var visited=new HashSet<Vector2Int>();
                var found=false;
                foreach(var fragment in fragments)
                {
                    float half=VoxelRubble.BlockSize(fragment.Size)*.5f;
                    for(int x=-1;x<=1 && !found;x+=2)for(int z=-1;z<=1 && !found;z+=2)
                    {
                        var sample=new Vector3(fragment.Position.x+x*half,point.y,fragment.Position.z+z*half);
                        if(!ground.TryColumn(sample.x,sample.z,out var column)||!visited.Add(column))continue;
                        var shift=sample-point;
                        found=ground.RaycastSurface(from+shift,point+shift,out hit);
                    }
                    if(found)break;
                }
                if(!found)return false;
            }
            offset=VoxelRubble.ImpactClearance(fragments,fragments.Count,hit.point,hit.normal);
            if(hit.normal.y>.999f)
            {
                // A voxel top is stepped. Clear every committed column touched
                // by each fragment's footprint, not only the center hit plane.
                var tops=new Dictionary<Vector2Int,float?>();
                float Surface(float x,float z)
                {
                    if(!ground.TryColumn(x,z,out var column))return float.NegativeInfinity;
                    if(!tops.TryGetValue(column,out var value))
                    {value=ground.TryColumnTop(column,out var top)?top:(float?)null;tops.Add(column,value);}
                    return value ?? float.NegativeInfinity;
                }
                foreach(var fragment in fragments)
                {
                    float half=VoxelRubble.BlockSize(fragment.Size)*.5f;
                    var p=fragment.Position;
                    var top=Mathf.Max(Mathf.Max(Surface(p.x-half,p.z-half),Surface(p.x-half,p.z+half)),
                        Mathf.Max(Surface(p.x+half,p.z-half),Surface(p.x+half,p.z+half)));
                    offset.y=Mathf.Max(offset.y,top+.005f-(p.y-half));
                }
            }
            for(int i=0;i<fragments.Count;i++)
            {var fragment=fragments[i];fragments[i]=new VoxelFragment(fragment.Position+offset,fragment.Size,fragment.Color);}
            return true;
        }
        internal void EmitNativeImpact(NativeCampaignSession.VoxelImpact impact)
        {
            var point=new Vector3(impact.x,impact.z,impact.y)/64f;
            var velocity=ImpactVelocity(impact);
            var normal=ImpactNormal(impact);
            PrepareEmissionContact(contact,view.transform.position,patch,rubble.ActivePositions,terrain.Revision,point);
            const float size=.07f;var fragments=new List<VoxelFragment>();
            for(int z=-4;z<=4;z++)for(int y=-4;y<=4;y++)for(int x=-4;x<=4;x++)
                if(x*x+y*y+z*z<=16)fragments.Add(new VoxelFragment(point+new Vector3(x,y,z)*size,size,new Color32(255,(byte)(70+(x*x+y*y+z*z)*7),12,255)));
            if(normal.sqrMagnitude<.000001f && !impact.has_target &&
                TryUnknownGroundClearance(ground,new Vector3(impact.from_x,impact.from_z,impact.from_y)/64f,point,fragments,out var offset))
            {
                if(ReceiptLogging)Debug.Log($"MC_VOXEL_IMPACT_GROUND_CLEARANCE sequence={impact.sequence} offset={offset.ToString("F3")}");
            }
            rubble.EmitImpact(fragments,velocity,point,normal);impacts++;
            Debug.Log($"MC_VOXEL_NATIVE_IMPACT sequence={impact.sequence} slot={impact.projectile_slot} model={impact.model} point={point} geometry_changed={impact.geometry_changed}");
            if(ReceiptLogging)Debug.Log($"MC_VOXEL_RECEIPT kind=impact generation={worldGeneration} world_epoch={worldEpoch} tick={nativeTick} sequence={impact.sequence} slot={impact.projectile_slot} slot_generation={impact.slot_generation} target={impact.target_slot} has_target={(impact.has_target?1:0)} revision={impact.revision} geometry_changed={(impact.geometry_changed?1:0)}");
        }
        internal void Render()
        {
            if(!ready)return;
            long now=Stopwatch.GetTimestamp();double frameMs=(now-wall)*1000.0/Stopwatch.Frequency;if(Time.time-started>1)frames.Add(frameMs);wall=now;
            if(ModernCapture.Measure)view.GetComponent<ModernCapture>()?.SampleWorldFrame(nativeTick,rubble.ActiveBodies,frameMs,Time.time-started,Time.realtimeSinceStartup-startedWall);
            float t=Time.time-started;
            if(automatic&&!characterReview&&!nativeCamera){
                if(t<8)Look(home+new Vector3(-24+t*.5f,13,-31),home+Vector3.up*5);
                else if(t<17)Look(home+new Vector3(-19,9,-24),home+new Vector3(0,4,-10));
                else {var target=home+new Vector3(12,0,-10);target.y=terrain.SampleHeight(target.x,target.z);Look(new Vector3(home.x+13,target.y+8,home.z-18),target);}
            } else if(!automatic) {
                if(Input.GetKeyDown(KeyCode.B)){freeFlight=true;flightPositioned=false;Debug.Log("MC_VOXEL_VIEW free");}
                if(Input.GetKeyDown(KeyCode.F2)){freeFlight=false;Debug.Log("MC_VOXEL_VIEW game");}
                if(freeFlight){
                    if(!flightPositioned){Look(home+new Vector3(-24,12,-30),home+Vector3.up*5);flightPosition=view.transform.position;yaw=view.transform.eulerAngles.y;pitch=view.transform.eulerAngles.x;flightPositioned=true;}
                    if(Input.GetMouseButton(1)){yaw+=Input.GetAxis("Mouse X")*2;pitch=Mathf.Clamp(Mathf.DeltaAngle(0,pitch)-Input.GetAxis("Mouse Y")*2,-85,85);}
                    var rotation=Quaternion.Euler(pitch,yaw,0);
                    var move=new Vector3((Input.GetKey(KeyCode.D)?1:0)-(Input.GetKey(KeyCode.A)?1:0),(Input.GetKey(KeyCode.E)?1:0)-(Input.GetKey(KeyCode.Q)?1:0),(Input.GetKey(KeyCode.W)?1:0)-(Input.GetKey(KeyCode.S)?1:0));
                    flightPosition+=rotation*move*Mathf.Min(Time.deltaTime,.05f)*(Input.GetKey(KeyCode.LeftShift)?24:10);
                    view.transform.SetPositionAndRotation(flightPosition,rotation);
                }
                if(Input.GetKeyDown(KeyCode.N))lighting.SetLighting(lightingPreset= (lightingPreset+1)%3);
            }
            if(automatic&&t>duration&&!capture){Report();Application.Quit();}
            contact.Update(view.transform.position,patch,rubble.ActivePositions,terrain.Revision);
        }
        private void Look(Vector3 from,Vector3 at){view.transform.SetPositionAndRotation(from,Quaternion.LookRotation(at-from));view.fieldOfView=55;}
        internal bool Explode(IReadOnlyList<VoxelFragment> cells,Vector3 velocity,Vector3 position,ushort slot,ushort model,uint slotGeneration,ushort sourceId)
        { if(!ready||cells.Count==0)return false;
            int buried=0;float depth=0;foreach(var c in cells){float d=terrain.SampleHeight(c.Position.x,c.Position.z)-c.Position.y;if(d>0){buried++;depth=Mathf.Max(depth,d);}}
            Debug.Log($"MC_VOXEL_DEATH_CONTACT position={position} velocity={velocity} buried={buried} depth={depth:F3} camera={view.transform.position}");
            PrepareEmissionContact(contact,view.transform.position,patch,rubble.ActivePositions,terrain.Revision,position);
            if(!rubble.EmitCharacter(cells,velocity,position,180,terrain.SampleHeight))return false;
            deaths++;Debug.Log($"MC_VOXEL_NATIVE_DEATH tick={nativeTick} slot={slot} model={model} cells={cells.Count} generation={worldEpoch} slot_generation={slotGeneration} source={sourceId}");return true; }
        internal void Draw()
        {
            if(!ready)return;
            var style=new GUIStyle(GUI.skin.label){fontSize=Mathf.Max(16,Screen.height/48)};
            GUI.Label(new Rect(20,18,Screen.width-40,38),"MODERN · FINE VOXELS · LIVE GAME BEHAVIOUR",style);
            string detail=characterReview?"96-cell character skins · original rigs and native behaviour":"25 cm bricks · 12.5 cm ground · Space: cast · F5: save · P: pause · B/F2: view";
            GUI.Label(new Rect(20,Screen.height-45,Screen.width-40,38),$"{detail} · tick {nativeTick}",style);
            if(!automatic&&editing)GUI.Label(new Rect(20,56,500,30),"Applying native destruction",style);
        }
        private IEnumerator Record()
        {
            for(int i=0;i<Mathf.RoundToInt(duration*30);i++){
                yield return new WaitForEndOfFrame();var texture=ScreenCapture.CaptureScreenshotAsTexture();
                File.WriteAllBytes(Path.Combine(output,"frames",$"frame-{i:D5}.jpg"),texture.EncodeToJPG(91));Destroy(texture);
            }
            Report();Application.Quit();
        }
        [Serializable] private sealed class Result { public string mode,device,resolution;public int impacts,goatDeaths,removedCells,groundRemoved,houseRemoved,completedEdits,peakBodies,contacts,rebounds,activeBodies,sleeping,falling,dropped,sampled,unrendered,forcedFade,belowTerrain,belowTerrainImpact,belowTerrainCharacter,belowTerrainOther;public bool editing;public float craterDepth;public ulong firstNativeTick,nativeTick;public double p50Ms,p95Ms,p99Ms,maxMs,maxEditStepMs,maxPhysicsMs,warmPhysicsMaxMs,physicsMeanMs,physicsP95Ms,wallSeconds; }
        private void Report()
        {
            var sorted=frames.OrderBy(x=>x).ToArray();double P(float p)=>sorted.Length==0?0:sorted[Mathf.Min(sorted.Length-1,(int)(p*(sorted.Length-1)))];
            var r=new Result{mode=capture?"actual player capture; timings include readback":"unthrottled Modern native gameplay",device=SystemInfo.graphicsDeviceName,resolution=$"{Screen.width}x{Screen.height}",impacts=impacts,goatDeaths=deaths,removedCells=removed,peakBodies=rubble.PeakBodies,contacts=rubble.ContactEvents,rebounds=rubble.ReboundEvents,firstNativeTick=firstNativeTick,nativeTick=nativeTick,p50Ms=P(.5f),p95Ms=P(.95f),p99Ms=P(.99f),maxMs=P(1),maxEditStepMs=maxEditMs};
            r.maxPhysicsMs=maxPhysicsMs;
            r.groundRemoved=groundRemoved;r.houseRemoved=houseRemoved;r.completedEdits=completedEdits;r.editing=editing;r.craterDepth=craterDepth;
            physicsSamples.Sort();r.warmPhysicsMaxMs=warmPhysicsMaxMs;r.physicsMeanMs=physicsTurns>0?physicsTotalMs/physicsTurns:0;r.physicsP95Ms=physicsSamples.Count>0?physicsSamples[(int)((physicsSamples.Count-1)*.95)]:0;r.wallSeconds=Time.realtimeSinceStartup-startedWall;
            r.activeBodies=rubble.ActiveBodies;r.sleeping=rubble.SleepingBodies;r.falling=rubble.FallingBodies;r.dropped=rubble.DroppedEmissions;r.sampled=rubble.SampledCells;r.unrendered=rubble.UnrenderedCells;r.forcedFade=rubble.ForcedFadePieces;
            float ContactHeight(float x,float z)=>x>=patch.min.x&&x<=patch.max.x&&z>=patch.min.z&&z<=patch.max.z?float.NegativeInfinity:terrain.SampleHeight(x,z);
            r.belowTerrainImpact=rubble.CountBelowSurface(ContactHeight,VoxelRubble.RubbleKind.Impact);
            r.belowTerrainCharacter=rubble.CountBelowSurface(ContactHeight,VoxelRubble.RubbleKind.Character);
            r.belowTerrainOther=rubble.CountBelowSurface(ContactHeight,VoxelRubble.RubbleKind.Other);
            r.belowTerrain=r.belowTerrainImpact+r.belowTerrainCharacter+r.belowTerrainOther;
            File.WriteAllText(Path.Combine(output,"report.json"),JsonUtility.ToJson(r,true));
            ModernCapture.WriteWorldProfile(output);
        }
        private void OnDestroy()
        {
            if(ready)Debug.Log($"MC_VOXEL_MODERN_EXIT camera={view.transform.position.ToString("F3")} impacts={impacts} tick={nativeTick}");
            if(Active==this)Active=null;
            if(ownsPhysics&&--physicsOwners==0){Physics.simulationMode=originalSimulation;Physics.defaultSolverIterations=originalSolver;Physics.defaultSolverVelocityIterations=originalVelocitySolver;}
            ResetProjection();if(terrain!=null)terrain.VoxelPatch=null;lighting?.Dispose();if(surface!=null)Destroy(surface);if(groundMaterial!=null)Destroy(groundMaterial);
        }
    }
}
