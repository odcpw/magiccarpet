using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using Unity.Profiling;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Debug = UnityEngine.Debug;

namespace MagicCarpet.Native.Voxels
{
    // An isolated local-physics experiment. Never opens or modifies a campaign/save.
    public sealed class VoxelSandbox : MonoBehaviour
    {
        [Serializable] private sealed class Building { public int source_slot, build_index; public float x,z,base_height,width,depth,height; }
        [Serializable] private sealed class Inputs { public int[] heights; public Building building; public ModernBuildingMeshes.Profile profile; }
        [Serializable] private sealed class Sample { public float time; public double frameMs, physicsMs, cpuMainMs,cpuRenderMs,presentMs,gpuMs,goatUpdateMs; public string phase; public int bodies, pending, sleeping, contacts, rebounds; }
        [Serializable] private sealed class Report {
            public string mode, device, resolution, diagnostic; public float voxelSize; public bool physicsRecorderAvailable, frameTimingsAvailable;
            public int frameCount, impacts, goatDeaths, removedCells, unsupportedCells, peakBodies, settled, forcedFade, emitted, disposed, contacts, rebounds, singleClusterEmissions, emissions;
            public double partitionMs, meshMs, spawnMs;
            public double p50Ms,p95Ms,p99Ms,maxMs; public Sample[] samples;
        }
        private Inputs input;
        private Material surface;
        private Mesh backgroundMesh;
        private VoxelVolume terrain, house;
        private readonly VoxelGoat[] goats = new VoxelGoat[3];
        private readonly Vector3[] lastGoat = new Vector3[3];
        private readonly float[] fallVelocity = new float[3];
        private readonly int[] resolutions = {24,40,64};
        private readonly List<Sample> samples = new List<Sample>();
        private Camera view;
        private VoxelRubble rubble;
        private VoxelFireball fireball;
        private GameObject content;
        private float cell=.25f, elapsed, yaw, pitch, cooldown;
        private int frame, nextShot, impacts, goatDeaths, removedCells, unsupportedCells;
        private int goatTarget=1;
        private bool automatic, capture, finished;
        private string output, phase="Explore", caption="LEVEL 1 / VOXEL SANDBOX";
        private string diagnostic;
        private readonly FrameTiming[] frameTiming=new FrameTiming[1];
        private long lastWall;
        private ProfilerRecorder physics;
        private GUIStyle heading, subtitle, small;
        private const float Duration=34f;
        private const float TerrainBase=-26f;

        private static string Option(string name) {
            var args=Environment.GetCommandLineArgs(); int i=Array.IndexOf(args,name);
            return i<0 ? null : i+1<args.Length ? args[i+1] : throw new ArgumentException(name+" needs a value");
        }
        private void Awake()
        {
            try {
                input=JsonUtility.FromJson<Inputs>(Resources.Load<TextAsset>("ModernCharacters/VoxelSandbox/Level1").text);
                if(input.heights.Length!=65536) throw new InvalidDataException("Invalid Level 1 terrain");
                var size=Option("-mc-voxel-size"); if(size!=null) cell=float.Parse(size,CultureInfo.InvariantCulture);
                var target=Option("-mc-voxel-goat-target"); if(target!=null)goatTarget=int.Parse(target,CultureInfo.InvariantCulture);
                if(goatTarget<0||goatTarget>2)throw new ArgumentException("Goat target must be 0, 1 or 2");
                if(cell<.2f || cell>1f) throw new ArgumentException("Voxel size must be .2–1 scene units");
                output=Option("-mc-voxel-capture"); capture=output!=null;
                if(output==null) output=Option("-mc-voxel-benchmark");
                automatic=output!=null;
                diagnostic=Option("-mc-voxel-diagnostic") ?? "none";
                if(automatic) {
                    if(Directory.Exists(output)) throw new IOException("Evidence output must be new");
                    Directory.CreateDirectory(output);
                    if(capture) { Directory.CreateDirectory(Path.Combine(output,"frames")); Time.captureFramerate=30; }
                }
                GraphicsSettings.defaultRenderPipeline=Resources.Load<UniversalRenderPipelineAsset>("ModernPipeline");
                QualitySettings.renderPipeline=null; QualitySettings.vSyncCount=0; Application.targetFrameRate=-1;
                Time.fixedDeltaTime=1f/120; Time.maximumDeltaTime=.1f;
                Physics.defaultSolverIterations=8; Physics.defaultSolverVelocityIterations=2;
                surface=new Material(Shader.Find("MagicCarpet/VoxelSurface"));
                RenderSettings.fog=true; RenderSettings.fogMode=FogMode.ExponentialSquared; RenderSettings.fogDensity=.004f;
                RenderSettings.fogColor=new Color(.52f,.65f,.72f);
                RenderSettings.ambientMode=AmbientMode.Trilight;
                RenderSettings.ambientSkyColor=new Color(.48f,.57f,.68f);
                RenderSettings.ambientEquatorColor=new Color(.32f,.34f,.33f);
                RenderSettings.ambientGroundColor=new Color(.15f,.13f,.10f);
                var sun=new GameObject("Late afternoon sun").AddComponent<Light>(); sun.type=LightType.Directional;
                sun.color=new Color(1,.89f,.70f); sun.intensity=1.3f; sun.shadows=LightShadows.Soft;
                sun.shadowBias=.05f; sun.shadowNormalBias=.25f; sun.transform.rotation=Quaternion.Euler(40,-38,0); RenderSettings.sun=sun;
                var skyShader=Shader.Find("Skybox/Procedural");
                if(skyShader!=null) { var sky=new Material(skyShader); sky.SetFloat("_AtmosphereThickness",.7f); sky.SetColor("_GroundColor",new Color(.36f,.38f,.35f)); RenderSettings.skybox=sky; }
                view=new GameObject("Voxel sandbox camera").AddComponent<Camera>(); view.tag="MainCamera";
                view.nearClipPlane=.05f; view.farClipPlane=280; view.fieldOfView=48;
                view.backgroundColor=RenderSettings.fogColor; view.clearFlags=CameraClearFlags.Skybox;
                view.GetUniversalAdditionalCameraData().renderPostProcessing=false;
                BuildScene();
                if(diagnostic=="no-shadows")sun.shadows=LightShadows.None;
                if(diagnostic=="no-goats")foreach(var goat in goats)goat.Root.SetActive(false);
                if(diagnostic=="no-render")view.enabled=false;
                physics=ProfilerRecorder.StartNew(ProfilerCategory.Physics,"Physics.Simulate",1);
                lastWall=Stopwatch.GetTimestamp();
                if(capture) StartCoroutine(CaptureFrames());
                Debug.Log($"MC_VOXEL_READY cell={cell} sourceSlot={input.building.source_slot} mode={(capture?"capture":automatic?"benchmark":"interactive")}");
            } catch(Exception e) { Debug.LogException(e); Application.Quit(2); enabled=false; }
        }

        private float Height(float x,float z)
        {
            float tx=(input.building.x+x*2)/4, tz=(input.building.z+z*2)/4;
            int ix=Mathf.FloorToInt(tx), iz=Mathf.FloorToInt(tz); float a=tx-ix,b=tz-iz;
            float A(int xx,int zz)=>input.heights[(xx&255)+(zz&255)*256]*.25f-input.building.base_height*.5f;
            float aa=A(ix,iz),bb=A(ix+1,iz),cc=A(ix,iz+1),dd=A(ix+1,iz+1);
            if(((ix+iz)&1)==0) return a<b ? aa+(dd-cc)*a+(cc-aa)*b : aa+(bb-aa)*a+(dd-bb)*b;
            return a+b<1 ? aa+(bb-aa)*a+(cc-aa)*b : dd+(cc-dd)*(1-a)+(bb-dd)*(1-b);
        }
        private float Ground(float x,float z) => TerrainBase+Mathf.Floor((Height(x,z)-TerrainBase)/cell+.0001f)*cell;
        private static Color32 Soil(float x,float z,float depth)
        {
            float noise=Mathf.PerlinNoise(x*.21f+71,z*.21f+19);
            if(depth>.7f) return Color.Lerp(new Color(.28f,.23f,.16f),new Color(.45f,.38f,.27f),noise);
            return Color.Lerp(new Color(.25f,.29f,.13f),new Color(.45f,.45f,.23f),noise);
        }
        private void BuildScene()
        {
            content=new GameObject("Voxel world"); content.layer=8; content.transform.SetParent(transform,false);
            rubble=content.AddComponent<VoxelRubble>(); fireball=content.AddComponent<VoxelFireball>(); fireball.Rubble=rubble;
            // Fine volume only in the interactive area; distant map uses coarse exposed faces.
            float max=0; for(float z=-16;z<16;z+=1)for(float x=-12;x<20;x+=1) max=Mathf.Max(max,Height(x,z));
            var dims=new Vector3Int(Mathf.RoundToInt(32/cell),Mathf.CeilToInt((max-TerrainBase)/cell)+1,Mathf.RoundToInt(32/cell));
            terrain=new VoxelVolume(content.transform,new Vector3(-12,TerrainBase,-16),dims,cell,surface);
            for(int z=0;z<dims.z;z++) for(int x=0;x<dims.x;x++) {
                float wx=-12+(x+.5f)*cell,wz=-16+(z+.5f)*cell; float h=Height(wx,wz);
                int n=Mathf.Clamp(Mathf.FloorToInt((h-TerrainBase)/cell+.0001f),0,dims.y);
                for(int y=0;y<n;y++) terrain.SetCell(x,y,z,Soil(wx,wz,h-(TerrainBase+(y+1)*cell)));
            }
            terrain.Build(); BuildBackground(); BuildHouse();
            foreach(var collider in content.GetComponentsInChildren<MeshCollider>()) collider.gameObject.layer=8;
            Physics.SyncTransforms();
            var source=Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            for(int i=0;i<3;i++) {
                goats[i]=new VoxelGoat(source,Resources.Load<VoxelGoatAsset>("ModernCharacters/Goat/Voxels/Goat"+resolutions[i]),content.transform,1.2f);
                var collider=goats[i].Root.AddComponent<BoxCollider>(); collider.center=new Vector3(0,.43f,0); collider.size=new Vector3(.45f,.65f,1.05f);
                PositionGoat(i,0,0);
            }
            Aim(new Vector3(21,9,-15),new Vector3(3,2,0));
        }
        private void BuildHouse()
        {
            var p=input.profile; float w=input.building.width*.5f,d=input.building.depth*.5f;
            int nx=Mathf.RoundToInt(w/cell), nz=Mathf.RoundToInt(d/cell), ny=Mathf.CeilToInt(input.building.height*.5f/cell);
            house=new VoxelVolume(content.transform,new Vector3(-w/2,0,-d/2),new Vector3Int(nx,ny,nz),cell,surface);
            PopulateHouse(house,p,cell);
            house.Build();
        }
        internal static void PopulateHouse(VoxelVolume house,ModernBuildingMeshes.Profile p,float cell)
        {
            float w=(p.columns-1)*2,d=(p.rows-1)*2;
            int nx=Mathf.RoundToInt(w/cell),nz=Mathf.RoundToInt(d/cell),ny=Mathf.CeilToInt(p.heights.Max()*.5f/cell);
            float Top(float x,float z) {
                float u=x/2,v=z/2; int ix=Mathf.Min((int)u,p.columns-2), iz=Mathf.Min((int)v,p.rows-2); u-=ix;v-=iz;
                float a=p.heights[ix+iz*p.columns]*.5f,b=p.heights[ix+1+iz*p.columns]*.5f,c=p.heights[ix+(iz+1)*p.columns]*.5f,dd=p.heights[ix+1+(iz+1)*p.columns]*.5f;
                if(((ix+iz+p.diagonal_parity)&1)==0) return u<v ? a+(dd-c)*u+(c-a)*v : a+(b-a)*u+(dd-b)*v;
                return u+v<1 ? a+(b-a)*u+(c-a)*v : dd+(c-dd)*(1-u)+(b-dd)*(1-v);
            }
            for(int z=0;z<nz;z++)for(int x=0;x<nx;x++) {
                float xx=(x+.5f)*cell,zz=(z+.5f)*cell,top=Top(xx,zz);
                for(int y=0;y<ny;y++) {
                    float yy=(y+.5f)*cell;
                    if(yy>top || (yy<top-1.6f && yy>cell))continue;
                    // Openings are real missing cells through the shell.
                    bool door=Mathf.Abs(xx-w*.5f)<.65f && zz<2.2f && yy<2.2f;
                    bool window=(Mathf.Abs(zz-d*.5f-2)<.55f || Mathf.Abs(zz-d*.5f+2)<.55f) && yy>2.2f && yy<3.1f && (xx<2 || xx>w-2);
                    if(door||window) continue;
                    bool roof=top>5.8f;
                    float grain=Mathf.PerlinNoise(xx*5+4,zz*5+y*.29f);
                    Color color=roof?Color.Lerp(new Color(.19f,.24f,.27f),new Color(.32f,.37f,.37f),grain):Color.Lerp(new Color(.53f,.46f,.33f),new Color(.75f,.67f,.49f),grain);
                    house.SetCell(x,y,z,color);
                }
            }
        }
        private void BuildBackground()
        {
            var vertices=new List<Vector3>(); var colors=new List<Color32>(); var triangles=new List<int>();
            void Quad(Vector3 a,Vector3 b,Vector3 c,Vector3 d,Color32 color) {
                int n=vertices.Count; vertices.Add(a);vertices.Add(b);vertices.Add(c);vertices.Add(d);
                for(int j=0;j<4;j++)colors.Add(color);
                triangles.Add(n);triangles.Add(n+1);triangles.Add(n+2);triangles.Add(n);triangles.Add(n+2);triangles.Add(n+3);
            }
            const float step=2;
            for(float z=-120;z<120;z+=step)for(float x=-120;x<120;x+=step) {
                if(x>=-12&&x<20&&z>=-16&&z<16)continue;
                float h=Mathf.Floor(Height(x+1,z+1)*2)*.5f;
                var color=Soil(x,z,0);
                Quad(new Vector3(x,h,z),new Vector3(x,h,z+step),new Vector3(x+step,h,z+step),new Vector3(x+step,h,z),color);
                float hx=Mathf.Floor(Height(x+3,z+1)*2)*.5f,hz=Mathf.Floor(Height(x+1,z+3)*2)*.5f;
                if(h>hx) Quad(new Vector3(x+step,hx,z),new Vector3(x+step,h,z),new Vector3(x+step,h,z+step),new Vector3(x+step,hx,z+step),color);
                else if(h<hx) Quad(new Vector3(x+step,h,z+step),new Vector3(x+step,hx,z+step),new Vector3(x+step,hx,z),new Vector3(x+step,h,z),color);
                if(h>hz)Quad(new Vector3(x+step,hz,z+step),new Vector3(x+step,h,z+step),new Vector3(x,h,z+step),new Vector3(x,hz,z+step),color);
                else if(h<hz)Quad(new Vector3(x,h,z+step),new Vector3(x,hz,z+step),new Vector3(x+step,hz,z+step),new Vector3(x+step,h,z+step),color);
            }
            var mesh=new Mesh {name="Level 1 coarse distance terrain",indexFormat=IndexFormat.UInt32}; backgroundMesh=mesh; mesh.SetVertices(vertices);mesh.SetColors(colors);mesh.SetTriangles(triangles,0);mesh.RecalculateNormals();mesh.RecalculateBounds();
            var go=new GameObject("Level 1 horizon");go.transform.SetParent(content.transform,false);go.AddComponent<MeshFilter>().sharedMesh=mesh;go.AddComponent<MeshRenderer>().sharedMaterial=surface;go.AddComponent<MeshCollider>().sharedMesh=mesh;
        }
        private void PositionGoat(int i,float t,float dt)
        {
            if(goats[i]==null)return;
            // Identical distance-driven motion at each detail level; an authored sandbox path.
            float z=5*Mathf.Cos(t*.3f),x=13+i*2+.5f*Mathf.Sin(t*.3f);
            float ground=Physics.Raycast(new Vector3(x,30,z),Vector3.down,out var hit,80,1<<8)?hit.point.y:TerrainBase;
            float y=ground+.04f;
            if(dt>0 && lastGoat[i].y>y+.12f) {fallVelocity[i]+=Physics.gravity.y*dt;y=Mathf.Max(y,lastGoat[i].y+fallVelocity[i]*dt);}
            else fallVelocity[i]=0;
            var p=new Vector3(x,y,z);
            float distance=dt==0?0:Vector3.Distance(p,lastGoat[i]);
            float heading=Mathf.Atan2(.15f*Mathf.Cos(t*.3f),-1.5f*Mathf.Sin(t*.3f))*Mathf.Rad2Deg;
            goats[i].Root.transform.SetPositionAndRotation(p,Quaternion.Euler(0,heading,0));
            goats[i].Render(dt,distance);lastGoat[i]=p;
        }
        private void Aim(Vector3 eye,Vector3 target) {view.transform.position=eye;view.transform.LookAt(target);var e=view.transform.eulerAngles;yaw=e.y;pitch=e.x;}
        private void Launch(Vector3 origin,Vector3 target) {
            Vector3 delta=target-origin;float flight=delta.magnitude/15f;
            fireball.Launch(origin,delta/flight-Physics.gravity*flight*.5f,.48f,Mathf.Min(.16f,cell*.45f),Impact);
        }
        private void Impact(Vector3 point,Vector3 normal,float energy)
        {
            impacts++; var timer=Stopwatch.StartNew();
            for(int i=0;i<3;i++) if(goats[i]!=null && Vector3.Distance(goats[i].Root.transform.position+Vector3.up*.6f,point)<2f) {
                var cells=goats[i].SnapshotFragments(); var velocity=new Vector3(.15f*Mathf.Cos(elapsed*.3f),0,-1.5f*Mathf.Sin(elapsed*.3f));
                goats[i].Dispose();goats[i]=null;goatDeaths++;
                rubble.Emit(cells,velocity+Vector3.up,point,Mathf.Min(energy,3f));
                Debug.Log($"MC_VOXEL_GOAT_FRACTURE resolution={resolutions[i]} occupiedCells={cells.Count} groups={rubble.LastEmissionGroups} gridCandidates={rubble.LastGridCandidates} generalCandidates={rubble.LastGeneralCandidates} partitionMs={rubble.LastPartitionMilliseconds:F3} meshMs={rubble.LastMeshMilliseconds:F3} spawnMs={rubble.LastSpawnMilliseconds:F3}");
            }
            var removed=house.RemoveSphere(point-normal*.25f,1.5f); removedCells+=removed.Count;
            rubble.Emit(removed,normal*1.5f+Vector3.up,point,Mathf.Min(energy,5f));
            if(removed.Count>0) foreach(var island in house.DetachUnsupported()) {unsupportedCells+=island.Count;rubble.EmitConnected(island,Vector3.zero,point,0);}
            removed=terrain.RemoveSphere(point-normal*.6f,1.6f);removedCells+=removed.Count;
            rubble.Emit(removed,normal+Vector3.up,point,Mathf.Min(energy,3f));
            if(removed.Count>0) foreach(var island in terrain.DetachAffectedUnsupported()) {unsupportedCells+=island.Count;rubble.EmitConnected(island,Vector3.zero,point,0);}
            Debug.Log($"MC_VOXEL_IMPACT t={elapsed:F3} point={point} energy={energy:F3} removedTotal={removedCells} unsupported={unsupportedCells} goats={goatDeaths} editMs={timer.Elapsed.TotalMilliseconds:F3}");
        }
        private void Update()
        {
            if(finished)return;
            long wall=Stopwatch.GetTimestamp();double ms=(wall-lastWall)*1000.0/Stopwatch.Frequency;lastWall=wall;
            float dt=Time.deltaTime; elapsed+=dt;
            long goatStart=Stopwatch.GetTimestamp();
            if(diagnostic!="no-goats")for(int i=0;i<3;i++)PositionGoat(i,elapsed,dt);
            double goatMs=(Stopwatch.GetTimestamp()-goatStart)*1000.0/Stopwatch.Frequency;
            if(automatic) {
                Tour();
                FrameTimingManager.CaptureFrameTimings();FrameTimingManager.GetLatestTimings(1,frameTiming);var ft=frameTiming[0];
                if(elapsed>1) samples.Add(new Sample {time=elapsed,frameMs=ms,physicsMs=physics.Valid?physics.LastValue/1e6:-1,cpuMainMs=ft.cpuMainThreadFrameTime,cpuRenderMs=ft.cpuRenderThreadFrameTime,presentMs=ft.cpuMainThreadPresentWaitTime,gpuMs=ft.gpuFrameTime,goatUpdateMs=goatMs,phase=phase,bodies=rubble.ActiveBodies,pending=rubble.PendingEmissions,sleeping=rubble.SleepingBodies,contacts=rubble.ContactEvents,rebounds=rubble.ReboundEvents});
                if(elapsed>=Duration && !capture) Finish();
            } else Controls(dt);
            if(Input.GetKeyDown(KeyCode.F10)||Input.GetKeyDown(KeyCode.Escape)) {Debug.Log($"MC_VOXEL_EXIT camera={view.transform.position}");Application.Quit();}
        }
        private void Tour()
        {
            if(elapsed<6) {
                phase="resolution comparison";caption="ONE GOAT / THREE VOXEL SIZES";
                float z=5*Mathf.Cos(elapsed*.3f);
                Aim(new Vector3(24,9,z-7),new Vector3(15,4.6f,z));
            } else if(elapsed<14) {
                phase=elapsed<9?"gait":"goat impact / settle";caption=elapsed<9?"THE APPROVED GAIT / DISTANCE-DRIVEN":"GOAT / IMPACT, BOUNCE, SETTLE";
                Vector3 target=goats[goatTarget]!=null?goats[goatTarget].Root.transform.position+Vector3.up*.8f:lastGoat[goatTarget]+Vector3.up*.4f;
                Aim(target+new Vector3(4.8f,2.8f,-5.5f),target);
                if(elapsed>=9 && nextShot==0) {Launch(target+new Vector3(0,1,-5),target+Vector3.back*.2f);nextShot++;}
            } else if(elapsed<24) {
                phase=elapsed<15?"house quiet":"house impact / settle";caption="LEVEL 1 HOUSE / PHYSICAL FRACTURE";
                Aim(new Vector3(13,8,-16),new Vector3(0,2.5f,-1));
                if(elapsed>=15 && nextShot==1){Launch(new Vector3(10,4,-9),new Vector3(3,2.7f,-2));nextShot++;}
                if(elapsed>=18 && nextShot==2){Launch(new Vector3(9,6,-10),new Vector3(1,5.8f,-1));nextShot++;}
            } else {
                phase=elapsed<26?"terrain impact":"rubble settle / fade";caption=elapsed<28?"TERRAIN / A REAL CRATER":"RUBBLE / REST, THEN FADE";
                Vector3 target=new Vector3(16,Ground(16,-10),-10);
                Aim(target+new Vector3(7,7,-8),target);
                if(elapsed>=25 && nextShot==3){Launch(target+new Vector3(0,5,-5),target);nextShot++;}
            }
        }
        private void Controls(float dt)
        {
            if(Input.GetMouseButton(1)) {yaw+=Input.GetAxisRaw("Mouse X")*2;pitch=Mathf.Clamp(pitch-Input.GetAxisRaw("Mouse Y")*2,-85,85);view.transform.rotation=Quaternion.Euler(pitch,yaw,0);}
            Vector3 move=new Vector3(Input.GetAxisRaw("Horizontal"),(Input.GetKey(KeyCode.E)?1:0)-(Input.GetKey(KeyCode.Q)?1:0),Input.GetAxisRaw("Vertical"));
            view.transform.position+=view.transform.TransformDirection(move)*dt*(Input.GetKey(KeyCode.LeftShift)?15:6);
            cooldown-=dt;
            if((Input.GetMouseButton(0)||Input.GetKey(KeyCode.Space))&&cooldown<=0) {Launch(view.transform.position+view.transform.forward,view.transform.position+view.transform.forward*18);cooldown=.5f;}
            int preset=Input.GetKeyDown(KeyCode.Alpha1)?0:Input.GetKeyDown(KeyCode.Alpha2)?1:Input.GetKeyDown(KeyCode.Alpha3)?2:-1;
            if(preset>=0||Input.GetKeyDown(KeyCode.R)) {
                if(preset>=0)cell=new[]{.5f,1f/3,.25f}[preset];
                ResetScene();
            }
        }
        private void ResetScene() {
            for(int i=0;i<3;i++){goats[i]?.Dispose();goats[i]=null;} terrain.Dispose();house.Dispose();Destroy(backgroundMesh);Destroy(content);
            elapsed=0;nextShot=0;BuildScene();
            Debug.Log($"MC_VOXEL_RESET cell={cell}");
        }
        private IEnumerator CaptureFrames() {
            while(!finished) {
                yield return new WaitForEndOfFrame();
                var texture=new Texture2D(Screen.width,Screen.height,TextureFormat.RGB24,false);
                texture.ReadPixels(new Rect(0,0,Screen.width,Screen.height),0,0);texture.Apply();
                File.WriteAllBytes(Path.Combine(output,"frames",$"frame-{frame++:D5}.jpg"),texture.EncodeToJPG(92));Destroy(texture);
                if(elapsed>=Duration)Finish();
            }
        }
        private void Finish() {
            if(finished)return;finished=true;
            var sorted=samples.Select(s=>s.frameMs).OrderBy(x=>x).ToArray();
            double P(float q)=>sorted.Length==0?0:sorted[Mathf.Clamp((int)(q*(sorted.Length-1)),0,sorted.Length-1)];
            var report=new Report {mode=capture?"offline 30fps capture; timing not performance":"unthrottled standalone physics sandbox",device=SystemInfo.graphicsDeviceName,resolution=Screen.width+"x"+Screen.height,voxelSize=cell,frameCount=samples.Count,diagnostic=diagnostic,physicsRecorderAvailable=physics.Valid,frameTimingsAvailable=FrameTimingManager.IsFeatureEnabled(),
                impacts=impacts,goatDeaths=goatDeaths,removedCells=removedCells,unsupportedCells=unsupportedCells,peakBodies=rubble.PeakBodies,settled=rubble.SettledPieces,forcedFade=rubble.ForcedFadePieces,emitted=rubble.EmittedCells,disposed=rubble.DisposedCells,contacts=rubble.ContactEvents,rebounds=rubble.ReboundEvents,
                singleClusterEmissions=rubble.SingleClusterEmissions,emissions=rubble.EmissionCount,partitionMs=rubble.PartitionMilliseconds,meshMs=rubble.MeshMilliseconds,spawnMs=rubble.SpawnMilliseconds,
                p50Ms=P(.5f),p95Ms=P(.95f),p99Ms=P(.99f),maxMs=P(1),samples=samples.ToArray()};
            File.WriteAllText(Path.Combine(output,"report.json"),JsonUtility.ToJson(report,true));
            Debug.Log("MC_VOXEL_COMPLETE "+JsonUtility.ToJson(report).Substring(0,Mathf.Min(800,JsonUtility.ToJson(report).Length)));Application.Quit();
        }
        private void OnGUI() {
            float scale=Screen.width/1280f;GUI.matrix=Matrix4x4.Scale(new Vector3(scale,scale,1));float h=Screen.height/scale;
            if(heading==null) {
                heading=new GUIStyle(GUI.skin.label){fontSize=29,fontStyle=FontStyle.Bold,normal={textColor=Color.white}};
                subtitle=new GUIStyle(GUI.skin.label){fontSize=21,normal={textColor=new Color(.88f,.92f,.94f)}};
                small=new GUIStyle(GUI.skin.label){fontSize=17,normal={textColor=new Color(.85f,.88f,.88f)}};
            }
            GUI.color=new Color(.04f,.065f,.08f,.86f);GUI.DrawTexture(new Rect(0,0,1280,96),Texture2D.whiteTexture);GUI.DrawTexture(new Rect(0,h-72,1280,72),Texture2D.whiteTexture);GUI.color=Color.white;
            GUI.Label(new Rect(30,12,1200,42),caption,heading);
            GUI.Label(new Rect(30,55,1200,32),$"Level 1 · original house 26 · terrain / building cells {cell:F2} · {rubble?.ActiveBodies ?? 0} active rubble bodies",subtitle);
            GUI.Label(new Rect(30,h-65,1200,30),automatic?(elapsed<6?"Left to right: 24 / 40 / 64 cells per goat height · same model, texture and animation":"Gravity · collision · friction · sleeping · fade after rest") : "WASD / Q E fly · right-drag look · click / Space fire · 1 / 2 / 3 detail · R reset · F10 exit",subtitle);
            GUI.Label(new Rect(30,h-32,1200,28),"VOXEL EXPERIMENT · existing Modern game preserved · local physics, not campaign simulation",small);
        }
        private void OnDestroy() {physics.Dispose();for(int i=0;i<3;i++)goats[i]?.Dispose();terrain?.Dispose();house?.Dispose();if(surface!=null)Destroy(surface);if(backgroundMesh!=null)Destroy(backgroundMesh);}
    }
}
