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
    // Opt-in presentation/PhysX workbench over a running native campaign.
    // Character decisions remain native. Local terrain fractures are not saves.
    internal sealed class VoxelModernWorld : MonoBehaviour
    {
        internal static readonly bool Requested=Environment.GetCommandLineArgs().Contains("-mc-voxel-modern");
        internal static VoxelModernWorld Active { get; private set; }
        internal const float GroundCell=.125f, BrickCell=.25f;
        private ModernTerrainMesh terrain;
        private ModernVegetation vegetation;
        private readonly List<Vector3> clearedPlants=new();
        private Camera view;
        private ModernEnvironmentSandbox lighting;
        private VoxelSandbox.Inputs input;
        private VoxelVolume ground,house;
        private VoxelTerrainContact contact;
        private VoxelTerrainSeam seam;
        private VoxelRubble rubble;
        private VoxelFireball fireballs;
        private Material surface,groundMaterial;
        private GameObject physical;
        private Bounds patch;
        private Vector3 home;
        private bool ready,editing,shotPending;
        private float started,lastShot=-10;
        private int impacts,deaths,removed;
        private int groundRemoved,houseRemoved,completedEdits;
        private float craterDepth;
        private int lightingPreset;
        private string output;
        private bool capture,automatic;
        private bool freeFlight,flightPositioned;
        private bool characterReview;
        private float duration=28;
        private ulong firstNativeTick;
        private Vector3 flightPosition;
        private float yaw,pitch;
        private int shotStage;
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
        private ulong nativeTick;
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
            lighting.SetLighting(0); water.SetSandboxWind(1);
            output=Option("-mc-voxel-capture");capture=output!=null;
            output ??= Option("-mc-voxel-benchmark");automatic=output!=null;
            characterReview=Option("-mc-character-follow")!=null;
            if(Option("-mc-voxel-seconds") is string seconds)duration=float.Parse(seconds,CultureInfo.InvariantCulture);
            if(!float.IsFinite(duration)||duration<4||duration>60)throw new ArgumentException("Review duration must be 4..60 seconds");
            freeFlight=!automatic&&Option("-mc-character-follow")==null;
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
        internal void Observe(NativeCampaignSession session)
        {
            nativeTick=session.Tick;
            if(ready)return;
            surface=new Material(Shader.Find("MagicCarpet/VoxelSurface"));
            physical=new GameObject("Local fine voxel physics");physical.layer=8;physical.transform.SetParent(transform,false);
            rubble=physical.AddComponent<VoxelRubble>();fireballs=physical.AddComponent<VoxelFireball>();fireballs.Rubble=rubble;
            float low=float.PositiveInfinity,high=float.NegativeInfinity;
            for(float z=patch.min.z;z<=patch.max.z;z+=2)for(float x=patch.min.x;x<=patch.max.x;x+=2){float h=BaseHeight(x,z);low=Mathf.Min(low,h);high=Mathf.Max(high,h);}
            low=Mathf.Floor((low-3)/GroundCell)*GroundCell;
            var dims=new Vector3Int(256,Mathf.CeilToInt((high-low)/GroundCell)+1,320);
            ground=new VoxelVolume(physical.transform,new Vector3(patch.min.x,low,patch.min.z),dims,GroundCell,groundMaterial);
            for(int z=0;z<dims.z;z++)for(int x=0;x<dims.x;x++) {
                float wx=patch.min.x+(x+.5f)*GroundCell,wz=patch.min.z+(z+.5f)*GroundCell,h=BaseHeight(wx,wz);
                int n=Mathf.Clamp(Mathf.FloorToInt((h-low)/GroundCell),0,dims.y);
                for(int y=0;y<n;y++){
                    float depth=h-low-(y+1)*GroundCell;
                    float noise=Mathf.PerlinNoise(wx*.39f,wz*.39f);
                    Color c=depth>.3f?Color.Lerp(new Color(.30f,.25f,.18f),new Color(.48f,.39f,.28f),noise):Color.Lerp(new Color(.43f,.41f,.30f),new Color(.64f,.60f,.44f),noise);
                    ground.SetCell(x,y,z,c);
                }
            }
            ground.Build();
            var b=input.building;
            house=new VoxelVolume(physical.transform,home-new Vector3(b.width*.5f,0,b.depth*.5f),
                new Vector3Int(Mathf.RoundToInt(b.width/BrickCell),Mathf.CeilToInt(b.height/BrickCell),Mathf.RoundToInt(b.depth/BrickCell)),BrickCell,surface);
            VoxelSandbox.PopulateHouse(house,input.profile,BrickCell,2);house.Build();
            contact=new VoxelTerrainContact(physical.transform,terrain.SampleHeight);contact.Update(home,patch);
            seam=new VoxelTerrainSeam(physical.transform,patch,terrain.SampleHeight,groundMaterial);
            Physics.SyncTransforms();ready=true;started=Time.time;startedWall=Time.realtimeSinceStartup;firstNativeTick=nativeTick;wall=Stopwatch.GetTimestamp();
            if(capture)StartCoroutine(Record());
            Debug.Log($"MC_VOXEL_MODERN_READY native_tick={nativeTick} brick={BrickCell} terrain={GroundCell} source_house={b.source_slot} local_destruction=True");
        }
        private float BaseHeight(float x,float z)
        {
            // Original source height triangles, with validated BUILD stamps removed.
            float u=x/4,v=z/4;int ix=Mathf.FloorToInt(u),iz=Mathf.FloorToInt(v);u-=ix;v-=iz;
            float H(int xx,int zz)=>input.heights[(xx&255)+(zz&255)*256]*.5f;
            float a=H(ix,iz),b=H(ix+1,iz),c=H(ix,iz+1),d=H(ix+1,iz+1);
            float source=((ix+iz)&1)==0?(u<v?a+(d-c)*u+(c-a)*v:a+(b-a)*u+(d-b)*v):(u+v<1?a+(b-a)*u+(c-a)*v:d+(c-d)*(1-u)+(b-d)*(1-v));
            float edge=Mathf.Min(x-patch.min.x,patch.max.x-x,z-patch.min.z,patch.max.z-z);
            return Mathf.Lerp(terrain.SampleHeight(x,z),source,Mathf.SmoothStep(0,1,edge/3));
        }
        internal void Render()
        {
            if(!ready)return;
            long now=Stopwatch.GetTimestamp();if(Time.time-started>1)frames.Add((now-wall)*1000.0/Stopwatch.Frequency);wall=now;
            float t=Time.time-started;
            if(shotPending&&fireballs.ActiveProjectiles==0)shotPending=false;
            if(automatic&&!characterReview){
                if(t<8)Look(home+new Vector3(-24+t*.5f,13,-31),home+Vector3.up*5);
                else if(t<17)Look(home+new Vector3(-19,9,-24),home+new Vector3(0,4,-10));
                else {var target=home+new Vector3(12,0,-10);target.y=BaseHeight(target.x,target.z);Look(new Vector3(home.x+13,target.y+8,home.z-18),target);}
                if(shotStage==0&&t>8){Shoot(home+new Vector3(-2,4,-10));shotStage++;}
                if(shotStage==1&&t>12&&!editing){Shoot(home+new Vector3(2,9,-6));shotStage++;}
                if(shotStage==2&&t>20&&!editing){var p=home+new Vector3(12,0,-10);p.y=BaseHeight(p.x,p.z);Shoot(p);shotStage++;}
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
                if(Input.GetKeyDown(KeyCode.Mouse0)||Input.GetKeyDown(KeyCode.Space))Shoot(view.transform.position+view.transform.forward*80);
            }
            if(automatic&&t>duration&&!capture){Report();Application.Quit();}
            contact.Update(view.transform.position,patch,rubble.ActivePositions,terrain.Revision);
        }
        private void Look(Vector3 from,Vector3 at){view.transform.SetPositionAndRotation(from,Quaternion.LookRotation(at-from));view.fieldOfView=55;}
        private void Shoot(Vector3 target)
        {
            if(Time.time-lastShot<.3f)return;lastShot=Time.time;
            if(editing||shotPending){Debug.Log("MC_VOXEL_SHOT_BUSY");return;}
            var direction=(target-view.transform.position).normalized;
            shotPending=fireballs.Launch(view.transform.position+direction*.8f,direction*30,.32f,.065f,(point,normal,energy)=>{
                shotPending=false;impacts++;StartCoroutine(Fracture(point-normal*.45f,energy));
                Debug.Log($"MC_VOXEL_MODERN_IMPACT count={impacts} point={point}");
            });
            if(shotPending)Debug.Log("MC_VOXEL_SHOT launched");
        }
        private static float StaticHeight(Vector3 point)
        {
            // Ignore dynamic rubble: this probe verifies the actual recooked
            // terrain contact surface, not merely a removed-cell counter.
            var hits=Physics.RaycastAll(point+Vector3.up*30,Vector3.down,100,1<<8,QueryTriggerInteraction.Ignore);
            float height=float.NegativeInfinity;
            foreach(var hit in hits)if(hit.rigidbody==null)height=Mathf.Max(height,hit.point.y);
            return height;
        }
        private IEnumerator Fracture(Vector3 point,float energy)
        {
            editing=true;
            var volume=point.y>home.y+.5f&&Mathf.Abs(point.x-home.x)<input.building.width*.5f+1&&Mathf.Abs(point.z-home.z)<input.building.depth*.5f+1?house:ground;
            float before=volume==ground?StaticHeight(point):0;
            var edit=volume.BeginRemoveSphere(point,1.65f);
            while(!edit.Complete){edit.Step(1);yield return null;}
            removed+=edit.Removed.Count;maxEditMs=Math.Max(maxEditMs,edit.MaximumStepMilliseconds);
            if(volume==ground){
                groundRemoved+=edit.Removed.Count;float after=StaticHeight(point);if(float.IsFinite(before)&&float.IsFinite(after))craterDepth=Mathf.Max(craterDepth,before-after);
                if(edit.Removed.Count>0){clearedPlants.Add(point);vegetation.InvalidateFootprint(new Bounds(point,Vector3.one*7));}
            }
            else houseRemoved+=edit.Removed.Count;
            rubble.Emit(edit.Removed,Vector3.up*.6f,point,Mathf.Min(energy,400));
            var support=volume.BeginDetachAffectedUnsupported();
            while(!support.Complete){support.Step(128);yield return null;}
            foreach(var group in support.Groups)rubble.EmitConnected(group,Vector3.zero,point,120);
            maxEditMs=Math.Max(maxEditMs,support.MaximumStepMilliseconds);
            completedEdits++;
            Debug.Log($"MC_VOXEL_MODERN_EDIT shot={impacts} volume={(volume==ground?"ground":"house")} removed={edit.Removed.Count} chunks={edit.RebuiltChunks} crater_depth={craterDepth:F3} max_step_ms={edit.MaximumStepMilliseconds:F3} support_max_ms={support.MaximumStepMilliseconds:F3}");
            editing=false;
        }
        internal bool Explode(IReadOnlyList<VoxelFragment> cells,Vector3 velocity,Vector3 position,ushort slot,ushort model)
        { if(!ready||cells.Count==0)return false;
            int buried=0;float depth=0;foreach(var c in cells){float d=terrain.SampleHeight(c.Position.x,c.Position.z)-c.Position.y;if(d>0){buried++;depth=Mathf.Max(depth,d);}}
            Debug.Log($"MC_VOXEL_DEATH_CONTACT position={position} velocity={velocity} buried={buried} depth={depth:F3} camera={view.transform.position}");
            rubble.Emit(cells,velocity,position,180);deaths++;Debug.Log($"MC_VOXEL_NATIVE_DEATH tick={nativeTick} slot={slot} model={model} cells={cells.Count}");return true; }
        internal void Draw()
        {
            if(!ready)return;
            var style=new GUIStyle(GUI.skin.label){fontSize=Mathf.Max(16,Screen.height/48)};
            GUI.Label(new Rect(20,18,Screen.width-40,38),"MODERN · FINE VOXELS · LIVE GAME BEHAVIOUR",style);
            string detail=characterReview?"96-cell character skins · original rigs and native behaviour":"Brick 25 cm · local ground 12.5 cm · B: free flight · F2: game view";
            GUI.Label(new Rect(20,Screen.height-45,Screen.width-40,38),$"{detail} · tick {nativeTick}",style);
            if(!automatic&&(editing||shotPending))GUI.Label(new Rect(20,56,500,30),"Fireball / destruction in progress",style);
        }
        private IEnumerator Record()
        {
            for(int i=0;i<Mathf.RoundToInt(duration*30);i++){
                yield return new WaitForEndOfFrame();var texture=ScreenCapture.CaptureScreenshotAsTexture();
                File.WriteAllBytes(Path.Combine(output,"frames",$"frame-{i:D5}.jpg"),texture.EncodeToJPG(91));Destroy(texture);
            }
            Report();Application.Quit();
        }
        [Serializable] private sealed class Result { public string mode,device,resolution;public int impacts,goatDeaths,removedCells,groundRemoved,houseRemoved,completedEdits,peakBodies,contacts,rebounds,activeBodies,sleeping,falling,dropped,sampled,unrendered,forcedFade,belowTerrain;public bool editing;public float craterDepth;public ulong firstNativeTick,nativeTick;public double p50Ms,p95Ms,p99Ms,maxMs,maxEditStepMs,maxPhysicsMs,warmPhysicsMaxMs,physicsMeanMs,physicsP95Ms,wallSeconds; }
        private void Report()
        {
            var sorted=frames.OrderBy(x=>x).ToArray();double P(float p)=>sorted.Length==0?0:sorted[Mathf.Min(sorted.Length-1,(int)(p*(sorted.Length-1)))];
            var r=new Result{mode=capture?"actual player capture; timings include readback":"unthrottled Modern native gameplay",device=SystemInfo.graphicsDeviceName,resolution=$"{Screen.width}x{Screen.height}",impacts=impacts,goatDeaths=deaths,removedCells=removed,peakBodies=rubble.PeakBodies,contacts=rubble.ContactEvents,rebounds=rubble.ReboundEvents,firstNativeTick=firstNativeTick,nativeTick=nativeTick,p50Ms=P(.5f),p95Ms=P(.95f),p99Ms=P(.99f),maxMs=P(1),maxEditStepMs=maxEditMs};
            r.maxPhysicsMs=maxPhysicsMs;
            r.groundRemoved=groundRemoved;r.houseRemoved=houseRemoved;r.completedEdits=completedEdits;r.editing=editing;r.craterDepth=craterDepth;
            physicsSamples.Sort();r.warmPhysicsMaxMs=warmPhysicsMaxMs;r.physicsMeanMs=physicsTurns>0?physicsTotalMs/physicsTurns:0;r.physicsP95Ms=physicsSamples.Count>0?physicsSamples[(int)((physicsSamples.Count-1)*.95)]:0;r.wallSeconds=Time.realtimeSinceStartup-startedWall;
            r.activeBodies=rubble.ActiveBodies;r.sleeping=rubble.SleepingBodies;r.falling=rubble.FallingBodies;r.dropped=rubble.DroppedEmissions;r.sampled=rubble.SampledCells;r.unrendered=rubble.UnrenderedCells;r.forcedFade=rubble.ForcedFadePieces;
            r.belowTerrain=rubble.CountBelowSurface((x,z)=>x>=patch.min.x&&x<=patch.max.x&&z>=patch.min.z&&z<=patch.max.z?float.NegativeInfinity:terrain.SampleHeight(x,z));
            File.WriteAllText(Path.Combine(output,"report.json"),JsonUtility.ToJson(r,true));
        }
        private void OnDestroy()
        {
            if(ready)Debug.Log($"MC_VOXEL_MODERN_EXIT camera={view.transform.position.ToString("F3")} impacts={impacts} tick={nativeTick}");
            if(Active==this)Active=null;
            if(ownsPhysics&&--physicsOwners==0){Physics.simulationMode=originalSimulation;Physics.defaultSolverIterations=originalSolver;Physics.defaultSolverVelocityIterations=originalVelocitySolver;}
            ground?.Dispose();house?.Dispose();contact?.Dispose();seam?.Dispose();lighting?.Dispose();if(surface!=null)Destroy(surface);if(groundMaterial!=null)Destroy(groundMaterial);
        }
    }
}
